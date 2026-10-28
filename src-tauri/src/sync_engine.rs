use crate::local_db::{
    build_local_connection_string, ensure_local_database_exists,
    format_local_mysql_connection_error, sanitize_identifier, spawn_mysql_child,
};
use crate::models::{
    incremental_primary_key, LocalDbConfig, LogPayload, ProgressPayload, RemoteDbConfig, SshConfig,
    SyncFailure, SyncOptions, SyncResult, TableDiffInfo, TableInfo,
};
use crate::ssh_client::{establish_ssh_session, start_ssh_tunnel};
use sqlx::mysql::MySqlPoolOptions;
use sqlx::Row;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tauri::Emitter;

pub static USER_CANCEL_REQUESTED: AtomicBool = AtomicBool::new(false);
pub static ACTIVE_CHILD_PIDS: Mutex<Vec<u32>> = Mutex::new(Vec::new());

pub fn register_child_pid(pid: u32) {
    if let Ok(mut lock) = ACTIVE_CHILD_PIDS.lock() {
        if !lock.contains(&pid) {
            lock.push(pid);
        }
    }
}

pub fn unregister_child_pid(pid: u32) {
    if let Ok(mut lock) = ACTIVE_CHILD_PIDS.lock() {
        lock.retain(|&p| p != pid);
    }
}

pub fn kill_all_child_processes() {
    let pids = {
        if let Ok(mut lock) = ACTIVE_CHILD_PIDS.lock() {
            let p = lock.clone();
            lock.clear();
            p
        } else {
            Vec::new()
        }
    };
    for pid in pids {
        #[cfg(unix)]
        {
            let _ = std::process::Command::new("kill")
                .args(["-9", &pid.to_string()])
                .output();
        }
        #[cfg(windows)]
        {
            let _ = crate::process_utils::new_std_command("taskkill")
                .args(["/F", "/T", "/PID", &pid.to_string()])
                .output();
        }
    }
}

pub fn is_cancelled() -> bool {
    USER_CANCEL_REQUESTED.load(Ordering::SeqCst)
}

pub fn reset_cancel_state() {
    USER_CANCEL_REQUESTED.store(false, Ordering::SeqCst);
}

pub fn cancel_sync() {
    USER_CANCEL_REQUESTED.store(true, Ordering::SeqCst);
    kill_all_child_processes();
}

pub fn emit_log<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    log_type: &str,
    message: impl Into<String>,
) {
    let now = chrono::Local::now().format("%H:%M:%S").to_string();
    let payload = LogPayload {
        r#type: log_type.to_string(),
        message: message.into(),
        timestamp: now,
        details: None,
        table_name: None,
    };
    let _ = app.emit("sync-log", payload);
}

fn record_failure<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    failures: &Arc<Mutex<Vec<SyncFailure>>>,
    table_name: Option<&str>,
    stage: &str,
    message: impl Into<String>,
) {
    let message = message.into();
    let failure = SyncFailure {
        table_name: table_name.map(str::to_string),
        stage: stage.to_string(),
        message: message.clone(),
    };
    failures
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .push(failure);

    let table_prefix = table_name
        .map(|name| format!("[Tabel '{}'] ", name))
        .unwrap_or_default();
    let payload = LogPayload {
        r#type: "error".to_string(),
        message: format!("{}{} gagal", table_prefix, stage),
        timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
        details: Some(message),
        table_name: table_name.map(str::to_string),
    };
    let _ = app.emit("sync-log", payload);
}

fn table_has_failure(failures: &Arc<Mutex<Vec<SyncFailure>>>, table_name: &str) -> bool {
    failures
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .iter()
        .any(|failure| failure.table_name.as_deref() == Some(table_name))
}

fn table_has_no_data_to_sync(sync_data: bool, rows_synced: usize, has_failure: bool) -> bool {
    sync_data && rows_synced == 0 && !has_failure
}

fn progress_table_counts(
    completed_table_names: &HashSet<String>,
    skipped_table_names: &HashSet<String>,
    failures: &[SyncFailure],
) -> (usize, usize, usize, usize) {
    let failed_table_names: HashSet<&str> = failures
        .iter()
        .filter_map(|failure| failure.table_name.as_deref())
        .collect();
    let skipped_tables = skipped_table_names
        .iter()
        .filter(|table_name| !failed_table_names.contains(table_name.as_str()))
        .count();
    let successful_tables = completed_table_names
        .iter()
        .filter(|table_name| {
            !skipped_table_names.contains(*table_name)
                && !failed_table_names.contains(table_name.as_str())
        })
        .count();
    let processed_tables = completed_table_names
        .iter()
        .map(String::as_str)
        .chain(failed_table_names.iter().copied())
        .collect::<HashSet<_>>()
        .len();

    (
        processed_tables,
        successful_tables,
        failed_table_names.len(),
        skipped_tables,
    )
}

fn has_mysql_cli_errors(stderr: &str) -> bool {
    stderr.lines().any(|line| {
        let line = line.trim_start();
        line.starts_with("ERROR ") || line.starts_with("ERROR:")
    })
}

fn is_schema_mismatch_error(err: &str) -> bool {
    let lower = err.to_lowercase();
    lower.contains("unknown column")
        || lower.contains("column count doesn't match")
        || (lower.contains("table") && lower.contains("doesn't exist"))
        || lower.contains("doesn't have a default value")
        || lower.contains("data truncated")
        || lower.contains("cannot be null")
        || lower.contains("error 1054")
        || lower.contains("error 1136")
        || lower.contains("error 1146")
        || lower.contains("error 1364")
        || lower.contains("error 1265")
        || lower.contains("error 1048")
        || lower.contains("incorrect integer value")
        || lower.contains("incorrect decimal value")
        || lower.contains("incorrect date")
        || lower.contains("error 1366")
        || lower.contains("error 1292")
}

fn extract_clean_mysql_error(err: &str) -> String {
    for line in err.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("ERROR ")
            || trimmed.starts_with("ERROR:")
            || trimmed.contains("Unknown column")
            || trimmed.contains("doesn't exist")
        {
            return trimmed.to_string();
        }
    }
    err.lines().next().unwrap_or(err).trim().to_string()
}

fn resolve_sync_status(
    was_cancelled: bool,
    fatal_failure: bool,
    all_tables_processed: bool,
    has_failures: bool,
) -> &'static str {
    if was_cancelled {
        "cancelled"
    } else if fatal_failure || !all_tables_processed {
        "failed"
    } else if has_failures {
        "partial"
    } else {
        "success"
    }
}

fn normalize_mysql_timestamp(value: &str) -> Option<String> {
    chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S%.f")
        .ok()
        .map(|timestamp| timestamp.format("%Y-%m-%d %H:%M:%S%.f").to_string())
}

#[allow(dead_code)]
fn normalize_create_table_ddl(ddl: &str) -> String {
    // Extract column/index definition body between first '(' and last ')'
    let body = match (ddl.find('('), ddl.rfind(')')) {
        (Some(start), Some(end)) if start < end => &ddl[start + 1..end],
        _ => ddl,
    };

    let mut clean_body = String::with_capacity(body.len());
    for c in body.chars() {
        if c != '`' {
            clean_body.push(c.to_ascii_lowercase());
        }
    }

    let tokens: Vec<&str> = clean_body.split_whitespace().collect();
    let mut normalized_tokens = Vec::with_capacity(tokens.len());

    for token in tokens {
        // Strip integer display widths, e.g. int(11) -> int, bigint(20) -> bigint
        let clean = if let Some(idx) = token.find('(') {
            let prefix = &token[..idx];
            if matches!(
                prefix,
                "int" | "tinyint" | "smallint" | "mediumint" | "bigint"
            ) {
                if token.ends_with(',') {
                    format!("{},", prefix)
                } else {
                    prefix.to_string()
                }
            } else {
                token.to_string()
            }
        } else {
            token.to_string()
        };
        normalized_tokens.push(clean);
    }

    normalized_tokens.join(" ")
}

#[allow(dead_code)]
async fn get_table_create_ddl(
    pool: &sqlx::MySqlPool,
    table_name: &str,
) -> Result<Option<String>, String> {
    let exists: Option<String> = sqlx::query_scalar(
        "SELECT TABLE_NAME FROM information_schema.TABLES WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = ? AND TABLE_TYPE = 'BASE TABLE'",
    )
    .bind(table_name)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("Gagal memeriksa tabel lokal '{}': {}", table_name, e))?;

    if exists.is_none() {
        return Ok(None);
    }

    let safe_table = sanitize_identifier(table_name)?;
    let query = format!("SHOW CREATE TABLE {}", safe_table);
    let row = sqlx::query(&query)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("Gagal membaca skema tabel lokal '{}': {}", table_name, e))?;
    let ddl: String = row
        .try_get(1)
        .map_err(|e| format!("Gagal membaca DDL tabel lokal '{}': {}", table_name, e))?;
    Ok(Some(ddl))
}

#[allow(dead_code)]
fn build_chunk_where_clause(
    safe_pk: &str,
    current_last_id: Option<i128>,
    chunk_max: i128,
    update_filter: Option<(i128, &str, &str)>,
) -> String {
    let range_condition = match current_last_id {
        Some(last_id) => format!("{} > {} AND {} <= {}", safe_pk, last_id, safe_pk, chunk_max),
        None => format!("{} <= {}", safe_pk, chunk_max),
    };
    let condition = match update_filter {
        Some((watermark_id, updated_at_col, timestamp)) => format!(
            "({} > {} OR {} >= '{}') AND {}",
            safe_pk, watermark_id, updated_at_col, timestamp, range_condition
        ),
        None => range_condition,
    };
    format!("--where={}", condition)
}

fn check_importer_running<R: tauri::Runtime>(
    child: &mut tokio::process::Child,
    app: &tauri::AppHandle<R>,
    failures: &Arc<Mutex<Vec<SyncFailure>>>,
    fatal_failure: &AtomicBool,
    table_name: &str,
) -> bool {
    match child.try_wait() {
        Ok(None) => true,
        Ok(Some(status)) => {
            fatal_failure.store(true, Ordering::SeqCst);
            record_failure(
                app,
                failures,
                Some(table_name),
                "Importer MySQL lokal",
                format!(
                    "Proses berhenti sebelum seluruh input selesai (status {})",
                    status
                ),
            );
            false
        }
        Err(e) => {
            fatal_failure.store(true, Ordering::SeqCst);
            record_failure(
                app,
                failures,
                Some(table_name),
                "Memeriksa proses importer MySQL lokal",
                e.to_string(),
            );
            false
        }
    }
}

pub fn emit_progress<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    current_table_index: usize,
    total_tables: usize,
    completed_tables: usize,
    completed_table_names: &HashSet<String>,
    skipped_table_names: &HashSet<String>,
    current_table_name: &str,
    rows_synced_current_table: usize,
    total_synced_all_tables: usize,
    status: &str,
    start_time: Instant,
    failures: &Arc<Mutex<Vec<SyncFailure>>>,
) {
    let failures_snapshot = failures
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone();
    let (processed_tables, successful_tables, failed_tables, skipped_tables) =
        progress_table_counts(
            completed_table_names,
            skipped_table_names,
            &failures_snapshot,
        );
    let payload = ProgressPayload {
        current_table_index,
        total_tables,
        completed_tables,
        completed_table_names: completed_table_names.iter().cloned().collect(),
        skipped_table_names: skipped_table_names.iter().cloned().collect(),
        current_table_name: current_table_name.to_string(),
        rows_synced_current_table,
        total_synced_all_tables,
        processed_tables,
        successful_tables,
        failed_tables,
        skipped_tables,
        elapsed_ms: start_time.elapsed().as_millis() as u64,
        status: status.to_string(),
    };
    let _ = app.emit("sync-progress", payload);

    let pct = if total_tables > 0 {
        ((processed_tables as f64 / total_tables as f64) * 100.0).round().min(100.0) as u64
    } else {
        0
    };

    use tauri::Manager;
    for (_, win) in app.webview_windows() {
        let _ = win.set_progress_bar(tauri::window::ProgressBarState {
            progress: Some(pct),
            status: Some(tauri::window::ProgressBarStatus::Normal),
        });
        if status == "syncing" || status == "in_progress" {
            let _ = win.set_title(&format!("[{}%] ({} / {} Tabel) DB Sync Engine", pct, current_table_index, total_tables));
        }
    }
}

pub async fn test_ssh_and_remote_db(
    ssh_config: &SshConfig,
    remote_db_config: &RemoteDbConfig,
) -> Result<String, String> {
    // 1. Validate SSH connection
    let session = establish_ssh_session(ssh_config)?;

    // 2. Start tunnel to test remote MySQL directly
    let tunnel = start_ssh_tunnel(ssh_config, remote_db_config)?;
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let tunnel_conn_str = format!(
        "mysql://{}:{}@127.0.0.1:{}/{}",
        urlencoding::encode(&remote_db_config.username),
        urlencoding::encode(&remote_db_config.password),
        tunnel.local_port,
        urlencoding::encode(&remote_db_config.database)
    );

    let pool = MySqlPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(std::time::Duration::from_secs(8))
        .connect(&tunnel_conn_str)
        .await
        .map_err(|e| {
            format!(
                "Koneksi SSH berhasil, namun gagal terhubung ke MySQL remote: {}",
                e
            )
        })?;

    let row: (String,) = sqlx::query_as("SELECT VERSION()")
        .fetch_one(&pool)
        .await
        .map_err(|e| format!("Gagal query versi MySQL remote: {}", e))?;

    pool.close().await;
    drop(tunnel);
    drop(session);

    Ok(format!("SSH OK & Remote MySQL OK ({})", row.0))
}

pub async fn get_remote_tables(
    ssh_config: &SshConfig,
    remote_db_config: &RemoteDbConfig,
) -> Result<Vec<TableInfo>, String> {
    let tunnel = start_ssh_tunnel(ssh_config, remote_db_config)?;
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let tunnel_conn_str = format!(
        "mysql://{}:{}@127.0.0.1:{}/{}",
        urlencoding::encode(&remote_db_config.username),
        urlencoding::encode(&remote_db_config.password),
        tunnel.local_port,
        urlencoding::encode(&remote_db_config.database)
    );

    let pool = MySqlPoolOptions::new()
        .max_connections(2)
        .acquire_timeout(std::time::Duration::from_secs(10))
        .connect(&tunnel_conn_str)
        .await
        .map_err(|e| format!("Gagal terhubung ke remote DB via SSH tunnel: {}", e))?;

    let query = "
        SELECT 
            TABLE_NAME, 
            COALESCE(TABLE_ROWS, 0) AS total_rows, 
            COALESCE(DATA_LENGTH, 0) AS data_size_bytes 
        FROM information_schema.TABLES 
        WHERE TABLE_SCHEMA = DATABASE() 
          AND TABLE_TYPE = 'BASE TABLE' 
        ORDER BY TABLE_NAME ASC
    ";

    let rows = sqlx::query(query)
        .fetch_all(&pool)
        .await
        .map_err(|e| format!("Gagal membaca daftar tabel remote: {}", e))?;

    let mut tables = Vec::new();
    for r in rows {
        let name: String = r.try_get("TABLE_NAME").unwrap_or_default();
        let total_rows: i64 = r.try_get("total_rows").unwrap_or(0);
        let data_size_bytes: i64 = r.try_get("data_size_bytes").unwrap_or(0);

        if !name.is_empty() {
            tables.push(TableInfo {
                name,
                total_rows: total_rows.max(0) as u64,
                data_size_bytes: data_size_bytes.max(0) as u64,
            });
        }
    }

    pool.close().await;
    drop(tunnel);

    Ok(tables)
}

pub async fn compare_table_counts(
    ssh_config: &SshConfig,
    remote_db_config: &RemoteDbConfig,
    local_db_config: &LocalDbConfig,
    tables: Option<Vec<String>>,
) -> Result<Vec<TableDiffInfo>, String> {
    // 1. Fetch local table counts
    let local_counts: HashMap<String, i64> = {
        let conn_str = build_local_connection_string(local_db_config);
        let pool = MySqlPoolOptions::new()
            .max_connections(2)
            .acquire_timeout(std::time::Duration::from_secs(6))
            .connect(&conn_str)
            .await
            .map_err(|e| {
                format_local_mysql_connection_error("Gagal terhubung ke MySQL lokal", e)
            })?;

        let rows = sqlx::query(
            "SELECT TABLE_NAME, COALESCE(TABLE_ROWS, 0) AS total_rows 
             FROM information_schema.TABLES 
             WHERE TABLE_SCHEMA = DATABASE() AND TABLE_TYPE = 'BASE TABLE'",
        )
        .fetch_all(&pool)
        .await
        .map_err(|e| format!("Gagal membaca baris tabel lokal: {}", e))?;

        pool.close().await;

        let mut map = HashMap::new();
        for r in rows {
            let name: String = r.try_get("TABLE_NAME").unwrap_or_default();
            let total: i64 = r.try_get("total_rows").unwrap_or(0);
            if !name.is_empty() {
                map.insert(name, total);
            }
        }
        map
    };

    // 2. Fetch remote table counts via SSH tunnel
    let remote_counts: HashMap<String, i64> = {
        let tunnel = start_ssh_tunnel(ssh_config, remote_db_config)?;
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;

        let tunnel_conn_str = format!(
            "mysql://{}:{}@127.0.0.1:{}/{}",
            urlencoding::encode(&remote_db_config.username),
            urlencoding::encode(&remote_db_config.password),
            tunnel.local_port,
            urlencoding::encode(&remote_db_config.database)
        );

        let pool = MySqlPoolOptions::new()
            .max_connections(2)
            .acquire_timeout(std::time::Duration::from_secs(10))
            .connect(&tunnel_conn_str)
            .await
            .map_err(|e| format!("Gagal terhubung ke remote DB via SSH tunnel: {}", e))?;

        let rows = sqlx::query(
            "SELECT TABLE_NAME, COALESCE(TABLE_ROWS, 0) AS total_rows 
             FROM information_schema.TABLES 
             WHERE TABLE_SCHEMA = DATABASE() AND TABLE_TYPE = 'BASE TABLE' 
             ORDER BY TABLE_NAME ASC",
        )
        .fetch_all(&pool)
        .await
        .map_err(|e| format!("Gagal membaca daftar tabel remote: {}", e))?;

        pool.close().await;
        drop(tunnel);

        let mut map = HashMap::new();
        for r in rows {
            let name: String = r.try_get("TABLE_NAME").unwrap_or_default();
            let total: i64 = r.try_get("total_rows").unwrap_or(0);
            if !name.is_empty() {
                map.insert(name, total);
            }
        }
        map
    };

    // 3. Assemble comparison
    let target_names: Vec<String> = match tables {
        Some(list) if !list.is_empty() => list,
        _ => {
            let mut all_set = std::collections::BTreeSet::new();
            for k in remote_counts.keys() {
                all_set.insert(k.clone());
            }
            for k in local_counts.keys() {
                all_set.insert(k.clone());
            }
            all_set.into_iter().collect()
        }
    };

    let mut diffs = Vec::new();
    for name in target_names {
        let rem_opt = remote_counts.get(&name).copied();
        let loc_opt = local_counts.get(&name).copied();

        let (status, remote_rows, local_rows, row_diff) = match (rem_opt, loc_opt) {
            (Some(rem), None) => ("remote_only".to_string(), rem, 0, rem),
            (None, Some(loc)) => ("local_only".to_string(), 0, loc, -loc),
            (Some(rem), Some(loc)) => {
                let diff = rem - loc;
                let st = if diff == 0 {
                    "in_sync".to_string()
                } else {
                    "diverged".to_string()
                };
                (st, rem, loc, diff)
            }
            (None, None) => ("unknown".to_string(), 0, 0, 0),
        };

        diffs.push(TableDiffInfo {
            table_name: name,
            remote_rows,
            local_rows,
            row_diff,
            status,
        });
    }

    Ok(diffs)
}

fn format_number(n: usize) -> String {
    let s = n.to_string();
    let bytes = s.as_bytes();
    let mut result = String::with_capacity(s.len() + s.len() / 3);
    for (i, &b) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i) % 3 == 0 {
            result.push('.');
        }
        result.push(b as char);
    }
    result
}


#[derive(Default)]
struct MysqlDumpRowCounter {
    word: Vec<u8>,
    in_insert: bool,
    in_values: bool,
    quote: Option<u8>,
    escaped: bool,
    tuple_depth: usize,
    expect_tuple: bool,
    total_rows: usize,
}

impl MysqlDumpRowCounter {
    fn push(&mut self, chunk: &[u8]) -> usize {
        let previous_total = self.total_rows;
        for &byte in chunk {
            if let Some(quote) = self.quote {
                if self.escaped {
                    self.escaped = false;
                } else if byte == b'\\' {
                    self.escaped = true;
                } else if byte == quote {
                    self.quote = None;
                }
                continue;
            }

            if matches!(byte, b'\'' | b'"' | b'`') {
                self.finish_word();
                self.quote = Some(byte);
                continue;
            }

            if byte.is_ascii_alphanumeric() || byte == b'_' {
                self.word.push(byte.to_ascii_uppercase());
                continue;
            }

            self.finish_word();
            match byte {
                b'(' if self.in_values => {
                    if self.tuple_depth == 0 && self.expect_tuple {
                        self.total_rows += 1;
                        self.expect_tuple = false;
                    }
                    self.tuple_depth += 1;
                }
                b')' if self.in_values && self.tuple_depth > 0 => {
                    self.tuple_depth -= 1;
                }
                b',' if self.in_values && self.tuple_depth == 0 => {
                    self.expect_tuple = true;
                }
                b';' => {
                    self.in_insert = false;
                    self.in_values = false;
                    self.expect_tuple = false;
                    self.tuple_depth = 0;
                }
                _ => {}
            }
        }
        self.total_rows - previous_total
    }

    fn finish_word(&mut self) {
        if self.word.is_empty() {
            return;
        }

        match self.word.as_slice() {
            b"INSERT" | b"REPLACE" => {
                self.in_insert = true;
                self.in_values = false;
                self.expect_tuple = false;
                self.tuple_depth = 0;
            }
            b"VALUES" if self.in_insert => {
                self.in_values = true;
                self.expect_tuple = true;
                self.tuple_depth = 0;
            }
            _ => {}
        }
        self.word.clear();
    }
}

static DUMP_COMPRESSION_ARGS: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();

fn get_dump_compression_args(dump_bin: &str) -> &'static [String] {
    DUMP_COMPRESSION_ARGS.get_or_init(|| {
        let mut check_cmd = std::process::Command::new(dump_bin);
        check_cmd.arg("--help");
        if let Ok(output) = check_cmd.output() {
            let help_text = String::from_utf8_lossy(&output.stdout);
            if help_text.contains("--compression-algorithms") {
                return vec![
                    "--compression-algorithms=zstd,zlib,uncompressed".to_string(),
                    "--zstd-compression-level=1".to_string(),
                ];
            }
        }
        vec!["--compress".to_string()]
    })
}

async fn stream_dump_to_local_importer<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    tunnel_port: u16,
    remote_db_config: &RemoteDbConfig,
    table_name: &str,
    table_index: usize,
    where_clause: &str,
    use_replace: bool,
    local_stdin: &mut tokio::process::ChildStdin,
    shared_total_rows: &Arc<AtomicUsize>,
    total_tables: usize,
    completed_tables: &Arc<AtomicUsize>,
    completed_table_names: &Arc<Mutex<HashSet<String>>>,
    skipped_table_names: &Arc<Mutex<HashSet<String>>>,
    start_time: Instant,
    failures: &Arc<Mutex<Vec<SyncFailure>>>,
) -> Result<usize, String> {
    let dump_bin = if std::path::Path::new("/usr/bin/mariadb-dump").exists() {
        "mariadb-dump"
    } else {
        "mysqldump"
    };
    let mut dump_cmd = crate::process_utils::new_tokio_command(dump_bin);
    dump_cmd
        .arg("-h")
        .arg("127.0.0.1")
        .arg("-P")
        .arg(tunnel_port.to_string())
        .arg("-u")
        .arg(&remote_db_config.username)
        .arg("--no-create-info")
        .arg("--skip-triggers")
        .arg("--quick")
        .arg("--single-transaction");

    // Smart Low-CPU Compression (zstd level 1 with zlib / compress fallback)
    for arg in get_dump_compression_args(dump_bin) {
        dump_cmd.arg(arg);
    }

    dump_cmd
        .arg("--net-buffer-length=1048576")
        .arg("--max-allowed-packet=1073741824")
        .arg("--default-character-set=utf8mb4");

    if use_replace {
        dump_cmd.arg("--replace");
    }

    if !remote_db_config.password.is_empty() {
        dump_cmd.arg(format!("-p{}", remote_db_config.password));
    }

    if !where_clause.trim().is_empty() {
        dump_cmd.arg(where_clause);
    }

    dump_cmd.arg(&remote_db_config.database);
    dump_cmd.arg(table_name);

    dump_cmd.stdout(std::process::Stdio::piped());
    dump_cmd.stderr(std::process::Stdio::piped());

    let mut dump_child = dump_cmd
        .spawn()
        .map_err(|e| format!("Gagal menjalankan {}: {}", dump_bin, e))?;

    #[cfg(unix)]
    if let Some(ref stdout_ref) = dump_child.stdout {
        crate::process_utils::tune_pipe_capacity(stdout_ref, 512 * 1024);
    }

    let dump_pid = dump_child.id();
    if let Some(pid) = dump_pid {
        register_child_pid(pid);
    }

    let mut row_counter = MysqlDumpRowCounter::default();
    let mut stderr = dump_child
        .stderr
        .take()
        .ok_or_else(|| format!("Gagal membaca STDERR proses {}", dump_bin))?;
    let stderr_task = tokio::spawn(async move {
        use tokio::io::AsyncReadExt;
        let mut output = Vec::new();
        let mut buffer = [0u8; 8192];
        loop {
            let count = stderr.read(&mut buffer).await?;
            if count == 0 {
                break;
            }
            let remaining = (64 * 1024usize).saturating_sub(output.len());
            output.extend_from_slice(&buffer[..count.min(remaining)]);
        }
        Ok::<String, std::io::Error>(String::from_utf8_lossy(&output).trim().to_string())
    });

    if let Some(mut stdout) = dump_child.stdout.take() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        // 256 KiB high-throughput stream buffer
        let mut buffer = vec![0u8; 262144];
        let mut last_progress_rows = 0usize;
        let mut last_progress_time = Instant::now();

        loop {
            if is_cancelled() {
                let _ = dump_child.kill().await;
                break;
            }
            match stdout.read(&mut buffer).await {
                Ok(0) => break,
                Ok(n) => {
                    let chunk = &buffer[..n];
                    row_counter.push(chunk);

                    // Direct stream into worker's dedicated local MySQL STDIN pipe
                    if let Err(e) = local_stdin.write_all(chunk).await {
                        let _ = dump_child.kill().await;
                        let _ = dump_child.wait().await;
                        if let Some(pid) = dump_pid {
                            unregister_child_pid(pid);
                        }
                        let _ = stderr_task.await;
                        return Err(format!("Gagal menulis stream ke MySQL lokal: {}", e));
                    }

                    if row_counter.total_rows >= last_progress_rows + 5000
                        || (row_counter.total_rows > last_progress_rows
                            && last_progress_time.elapsed().as_millis() >= 500)
                    {
                        let delta = row_counter.total_rows - last_progress_rows;
                        let total_all = shared_total_rows.fetch_add(delta, Ordering::SeqCst) + delta;
                        last_progress_rows = row_counter.total_rows;
                        last_progress_time = Instant::now();

                        let c_names = completed_table_names
                            .lock()
                            .unwrap_or_else(|p| p.into_inner())
                            .clone();
                        let s_names = skipped_table_names
                            .lock()
                            .unwrap_or_else(|p| p.into_inner())
                            .clone();
                        emit_progress(
                            app,
                            table_index,
                            total_tables,
                            completed_tables.load(Ordering::Relaxed),
                            &c_names,
                            &s_names,
                            table_name,
                            row_counter.total_rows,
                            total_all,
                            "syncing",
                            start_time,
                            failures,
                        );
                    }
                }
                Err(e) => {
                    let _ = dump_child.kill().await;
                    let _ = dump_child.wait().await;
                    if let Some(pid) = dump_pid {
                        unregister_child_pid(pid);
                    }
                    let _ = stderr_task.await;
                    return Err(format!("Gagal membaca stream {}: {}", dump_bin, e));
                }
            }
        }

        if row_counter.total_rows > last_progress_rows {
            let delta = row_counter.total_rows - last_progress_rows;
            shared_total_rows.fetch_add(delta, Ordering::SeqCst);
        }
    }

    use tokio::io::AsyncWriteExt;
    if row_counter.total_rows > 0 {
        let _ = local_stdin.write_all(b"\nCOMMIT;\n").await;
        if let Err(e) = local_stdin.flush().await {
            return Err(format!("Gagal flush COMMIT data ke MySQL lokal: {}", e));
        }
    }

    let status = match dump_child.wait().await {
        Ok(status) => status,
        Err(e) => {
            if let Some(pid) = dump_pid {
                unregister_child_pid(pid);
            }
            return Err(format!("Gagal menunggu proses {}: {}", dump_bin, e));
        }
    };
    if let Some(pid) = dump_pid {
        unregister_child_pid(pid);
    }
    let stderr_output = stderr_task
        .await
        .map_err(|e| format!("Gagal mengambil keluaran error {}: {}", dump_bin, e))?
        .map_err(|e| format!("Gagal membaca STDERR {}: {}", dump_bin, e))?;

    if !is_cancelled() && !status.success() {
        let message = if stderr_output.is_empty() {
            format!("{} berhenti dengan status {}", dump_bin, status)
        } else {
            stderr_output
        };
        return Err(message);
    }

    Ok(row_counter.total_rows)
}

pub async fn run_sync_task<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    ssh_config: SshConfig,
    remote_db_config: RemoteDbConfig,
    local_db_config: LocalDbConfig,
    sync_options: SyncOptions,
) -> Result<SyncResult, String> {
    reset_cancel_state();
    let start_time = Instant::now();

    emit_log(
        &app,
        "info",
        "🚀 Memulai inisialisasi sinkronisasi DB over SSH...",
    );

    // 1. Ensure local database exists
    if let Err(e) = ensure_local_database_exists(&local_db_config).await {
        emit_log(
            &app,
            "error",
            format!("Gagal memverifikasi database lokal: {}", e),
        );
        return Err(e);
    }

    // 2. Open SSH port forward tunnel
    emit_log(
        &app,
        "info",
        format!("🔌 Menghubungkan SSH tunnel ke {}...", ssh_config.host),
    );
    let tunnel = match start_ssh_tunnel(&ssh_config, &remote_db_config) {
        Ok(t) => t,
        Err(e) => {
            emit_log(&app, "error", format!("Koneksi SSH tunnel gagal: {}", e));
            return Err(e);
        }
    };
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let tunnel_port = tunnel.local_port;
    let tunnel_conn_str = format!(
        "mysql://{}:{}@127.0.0.1:{}/{}",
        urlencoding::encode(&remote_db_config.username),
        urlencoding::encode(&remote_db_config.password),
        tunnel_port,
        remote_db_config.database
    );

    let remote_pool = match MySqlPoolOptions::new()
        .max_connections(6)
        .acquire_timeout(std::time::Duration::from_secs(15))
        .connect(&tunnel_conn_str)
        .await
    {
        Ok(p) => Arc::new(p),
        Err(e) => {
            emit_log(
                &app,
                "error",
                format!("Koneksi MySQL remote via tunnel gagal: {}", e),
            );
            return Err(e.to_string());
        }
    };

    let local_connection_string = build_local_connection_string(&local_db_config);
    let local_schema_pool = MySqlPoolOptions::new()
        .max_connections(2)
        .acquire_timeout(std::time::Duration::from_secs(8))
        .connect(&local_connection_string)
        .await
        .map_err(|e| {
            format_local_mysql_connection_error(
                "Gagal terhubung untuk memeriksa skema MySQL lokal",
                e,
            )
        })?;

    // 1. Batch query local schema column metadata
    let local_columns_query = "
        SELECT TABLE_NAME, COLUMN_NAME, COLUMN_TYPE, IS_NULLABLE
        FROM information_schema.COLUMNS
        WHERE TABLE_SCHEMA = DATABASE()
        ORDER BY TABLE_NAME, ORDINAL_POSITION ASC
    ";
    let local_col_rows = sqlx::query(local_columns_query)
        .fetch_all(&local_schema_pool)
        .await
        .unwrap_or_default();

    let mut local_schema_signatures: HashMap<String, Vec<(String, String, String)>> = HashMap::new();
    let mut existing_local_tables: HashSet<String> = HashSet::new();

    for r in local_col_rows {
        let t_name: String = r.try_get("TABLE_NAME").unwrap_or_default();
        let c_name: String = r.try_get("COLUMN_NAME").unwrap_or_default();
        let column_type: String = r.try_get("COLUMN_TYPE").unwrap_or_default();
        let is_nullable: String = r.try_get("IS_NULLABLE").unwrap_or_default();

        if !t_name.is_empty() && !c_name.is_empty() {
            existing_local_tables.insert(t_name.clone());
            local_schema_signatures
                .entry(t_name)
                .or_default()
                .push((c_name, column_type, is_nullable));
        }
    }
    let existing_local_tables = Arc::new(existing_local_tables);
    let local_schema_signatures = Arc::new(local_schema_signatures);

    // 2. Batch query remote schema metadata: PKs, updated_at columns, and full schema signatures
    let remote_columns_query = "
        SELECT TABLE_NAME, COLUMN_NAME, DATA_TYPE, COLUMN_TYPE, IS_NULLABLE, COLUMN_KEY
        FROM information_schema.COLUMNS 
        WHERE TABLE_SCHEMA = DATABASE()
        ORDER BY TABLE_NAME, ORDINAL_POSITION ASC
    ";
    let remote_col_rows = sqlx::query(remote_columns_query)
        .fetch_all(&*remote_pool)
        .await
        .map_err(|e| format!("Gagal membaca metadata kolom remote: {}", e))?;

    let mut pk_columns: HashMap<String, Vec<(String, String, String)>> = HashMap::new();
    let mut tables_with_updated_at: HashSet<String> = HashSet::new();
    let mut remote_schema_signatures: HashMap<String, Vec<(String, String, String)>> = HashMap::new();

    for r in remote_col_rows {
        let t_name: String = r.try_get("TABLE_NAME").unwrap_or_default();
        let c_name: String = r.try_get("COLUMN_NAME").unwrap_or_default();
        let data_type: String = r.try_get("DATA_TYPE").unwrap_or_default();
        let column_type: String = r.try_get("COLUMN_TYPE").unwrap_or_default();
        let is_nullable: String = r.try_get("IS_NULLABLE").unwrap_or_default();
        let column_key: String = r.try_get("COLUMN_KEY").unwrap_or_default();

        if !t_name.is_empty() && !c_name.is_empty() {
            if column_key == "PRI" {
                pk_columns
                    .entry(t_name.clone())
                    .or_default()
                    .push((c_name.clone(), data_type, column_type.clone()));
            }
            if c_name.eq_ignore_ascii_case("updated_at") {
                tables_with_updated_at.insert(t_name.clone());
            }
            remote_schema_signatures
                .entry(t_name)
                .or_default()
                .push((c_name, column_type, is_nullable));
        }
    }

    let pk_map: HashMap<String, String> = pk_columns
        .iter()
        .filter_map(|(table, columns)| {
            incremental_primary_key(columns).map(|column| (table.clone(), column))
        })
        .collect();
    let pk_map: HashMap<String, String> = pk_map
        .into_iter()
        .filter(|(table, _)| tables_with_updated_at.contains(table))
        .collect();
    let pk_map = Arc::new(pk_map);
    let tables_with_updated_at = Arc::new(tables_with_updated_at);
    let remote_schema_signatures = Arc::new(remote_schema_signatures);

    let (sync_timestamp, sync_timestamp_epoch_ms): (String, i64) = sqlx::query_as(
        "SELECT DATE_FORMAT(CURRENT_TIMESTAMP(6), '%Y-%m-%d %H:%i:%s.%f'), CAST(UNIX_TIMESTAMP(CURRENT_TIMESTAMP(6)) * 1000 AS SIGNED)",
    )
    .fetch_one(&*remote_pool)
    .await
    .map_err(|e| format!("Gagal membaca waktu server MySQL remote: {}", e))?;

    let total_tables = sync_options.tables.len();
    let sync_mode = sync_options.sync_mode.as_deref().unwrap_or("incremental");
    let is_fresh = sync_mode == "fresh";

    // Batched Fast-Check: identify tables with 0 changes in fast batches of UNION ALL across remote connection pool
    let mut pre_skipped_set = HashSet::new();
    if !is_fresh {
        if let Some(ref wm_map) = sync_options.incremental_watermarks {
            struct WatermarkCandidate {
                table_name: String,
                safe_table: String,
                cond: String,
            }

            let mut candidates = Vec::new();
            for table_item in &sync_options.tables {
                let table_name = &table_item.name;
                let table_is_fresh = is_fresh
                    || !existing_local_tables.contains(table_name)
                    || !local_schema_signatures.contains_key(table_name);

                if !table_is_fresh && table_item.sync_data {
                    if let Some(wm) = wm_map.get(table_name) {
                        let pk_col_opt = pk_map.get(table_name).cloned();
                        if let Some(ref pk_col) = pk_col_opt {
                            if wm.primary_key.eq_ignore_ascii_case(pk_col.as_str()) {
                                if let Ok(w_id) = wm.last_id.parse::<i128>() {
                                    if w_id > 0 {
                                        let safe_table = match sanitize_identifier(table_name) {
                                            Ok(s) => s,
                                            Err(_) => continue,
                                        };
                                        let safe_pk = match sanitize_identifier(pk_col) {
                                            Ok(s) => s,
                                            Err(_) => continue,
                                        };
                                        let updated_at_col = tables_with_updated_at
                                            .contains(table_name)
                                            .then(|| "`updated_at`".to_string());
                                        let watermark_timestamp = wm
                                            .last_sync_time
                                            .as_deref()
                                            .and_then(normalize_mysql_timestamp);

                                        let cond = match (updated_at_col, watermark_timestamp) {
                                            (Some(col), Some(ts)) => {
                                                format!("{} > {} OR {} >= '{}'", safe_pk, w_id, col, ts)
                                            }
                                            _ => format!("{} > {}", safe_pk, w_id),
                                        };

                                        candidates.push(WatermarkCandidate {
                                            table_name: table_name.clone(),
                                            safe_table,
                                            cond,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }

            let total_candidates = candidates.len();
            if !candidates.is_empty() {
                // Batch up to 40 tables per UNION ALL query to eliminate SSH round-trip latency
                const BATCH_SIZE: usize = 40;
                for chunk in candidates.chunks(BATCH_SIZE) {
                    let union_sql = chunk
                        .iter()
                        .map(|c| {
                            let escaped_name = c.table_name.replace('\'', "''");
                            format!(
                                "(SELECT '{}' AS _tbl FROM {} WHERE ({}) LIMIT 1)",
                                escaped_name, c.safe_table, c.cond
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(" UNION ALL ");

                    match sqlx::query(&union_sql).fetch_all(&*remote_pool).await {
                        Ok(rows) => {
                            let mut changed_tables = HashSet::new();
                            for r in rows {
                                if let Ok(tbl) = r.try_get::<String, _>("_tbl") {
                                    changed_tables.insert(tbl);
                                }
                            }
                            for c in chunk {
                                if !changed_tables.contains(&c.table_name) {
                                    pre_skipped_set.insert(c.table_name.clone());
                                }
                            }
                        }
                        Err(err) => {
                            eprintln!("[BatchWatermarkCheck] Batch query failed, falling back to individual checks: {}", err);
                            // Fallback to individual checks for this chunk
                            for c in chunk {
                                let single_sql = format!(
                                    "SELECT 1 FROM {} WHERE ({}) LIMIT 1",
                                    c.safe_table, c.cond
                                );
                                if let Ok(None) = sqlx::query(&single_sql).fetch_optional(&*remote_pool).await {
                                    pre_skipped_set.insert(c.table_name.clone());
                                }
                            }
                        }
                    }
                }

                if !pre_skipped_set.is_empty() {
                    emit_log(
                        &app,
                        "info",
                        format!(
                            "⚡ [Batch Fast-Check] {} dari {} tabel incremental tidak memiliki perubahan data baru (di-skip instan).",
                            pre_skipped_set.len(),
                            total_candidates
                        ),
                    );
                }
            }
        }
    }
    let pre_skipped_tables = Arc::new(pre_skipped_set);

    let pipeline_concurrency = sync_options
        .concurrency
        .map(|c| (c as usize).clamp(1, 4))
        .unwrap_or(if total_tables >= 2 { 2 } else { 1 })
        .min(total_tables.max(1));

    emit_log(
        &app,
        "info",
        format!(
            "🚀 Memulai Pipeline Sync ({} tabel, {} Concurrent Download & Local Import Workers, Mode: {})...",
            total_tables,
            pipeline_concurrency,
            if is_fresh { "Fresh Sync" } else { "Sync New & Update" }
        ),
    );

    let tables_queue = Arc::new(tokio::sync::Mutex::new(
        sync_options
            .tables
            .into_iter()
            .enumerate()
            .collect::<VecDeque<_>>(),
    ));
    let incremental_watermarks = Arc::new(sync_options.incremental_watermarks);
    let remote_db_cfg = Arc::new(remote_db_config);
    let local_db_cfg = Arc::new(local_db_config);
    let failures_consumer = Arc::new(Mutex::new(Vec::<SyncFailure>::new()));
    let failures = failures_consumer.clone();
    let fatal_failure = Arc::new(AtomicBool::new(false));
    let completed_tables = Arc::new(AtomicUsize::new(0));
    let total_rows_imported = Arc::new(AtomicUsize::new(0));
    let completed_table_names = Arc::new(Mutex::new(HashSet::<String>::new()));
    let skipped_table_names = Arc::new(Mutex::new(HashSet::<String>::new()));
    let schema_drift_tables = Arc::new(Mutex::new(HashSet::<String>::new()));

    // ---------------------------------------------------------------------------------
    // Concurrent Workers: Each worker owns 1 SSH Download + 1 Dedicated MySQL Local Importer
    // ---------------------------------------------------------------------------------
    let mut worker_handles = Vec::new();

    for worker_id in 0..pipeline_concurrency {
        let queue_c = tables_queue.clone();
        let pool_c = remote_pool.clone();
        let pk_map_c = pk_map.clone();
        let updated_at_tables_c = tables_with_updated_at.clone();
        let watermarks_c = incremental_watermarks.clone();
        let remote_cfg_c = remote_db_cfg.clone();
        let local_cfg_c = local_db_cfg.clone();
        let app_c = app.clone();
        let failures_c = failures_consumer.clone();
        let fatal_failure_c = fatal_failure.clone();
        let local_tables_c = existing_local_tables.clone();
        let remote_schema_c = remote_schema_signatures.clone();
        let local_schema_c = local_schema_signatures.clone();
        let pre_skipped_c = pre_skipped_tables.clone();
        let completed_tables_c = completed_tables.clone();
        let total_rows_imported_c = total_rows_imported.clone();
        let completed_table_names_c = completed_table_names.clone();
        let skipped_table_names_c = skipped_table_names.clone();
        let schema_drift_tables_c = schema_drift_tables.clone();
        let row_limit_opt = sync_options.row_limit;
        let total_tables_c = total_tables;
        let started_at_c = start_time;

        let handle = tokio::spawn(async move {
            let (mut local_child, mut local_stdin, mut local_stderr) =
                match spawn_mysql_child(&local_cfg_c) {
                    Ok(c) => c,
                    Err(e) => {
                        fatal_failure_c.store(true, Ordering::SeqCst);
                        record_failure(
                            &app_c,
                            &failures_c,
                            None,
                            &format!("Menjalankan importer MySQL lokal (Worker {})", worker_id + 1),
                            &e,
                        );
                        return Err(e);
                    }
                };
            let local_pid = local_child.id();
            if let Some(pid) = local_pid {
                register_child_pid(pid);
            }

            let stderr_task = tokio::spawn(async move {
                use tokio::io::AsyncReadExt;
                let mut output = Vec::new();
                let mut buffer = [0u8; 8192];
                loop {
                    let count = local_stderr.read(&mut buffer).await?;
                    if count == 0 {
                        break;
                    }
                    let remaining = (64 * 1024usize).saturating_sub(output.len());
                    output.extend_from_slice(&buffer[..count.min(remaining)]);
                }
                Ok::<String, std::io::Error>(String::from_utf8_lossy(&output).trim().to_string())
            });

            use tokio::io::AsyncWriteExt;
            let prelude = "SET SESSION sql_log_bin=0;\n\
                           SET SESSION foreign_key_checks=0;\n\
                           SET SESSION unique_checks=0;\n\
                           SET SESSION autocommit=0;\n\
                           SET SESSION sql_mode='';\n\
                           SET SESSION net_read_timeout=600;\n\
                           SET SESSION net_write_timeout=600;\n\
                           SET SESSION wait_timeout=600;\n\
                           SET SESSION lock_wait_timeout=600;\n\
                           SET SESSION innodb_lock_wait_timeout=600;\n";
            if let Err(e) = local_stdin.write_all(prelude.as_bytes()).await {
                fatal_failure_c.store(true, Ordering::SeqCst);
                record_failure(
                    &app_c,
                    &failures_c,
                    None,
                    &format!("Menyiapkan sesi MySQL lokal (Worker {})", worker_id + 1),
                    e.to_string(),
                );
            }
            let _ = local_stdin.flush().await;

            let mut touched_tables = HashSet::new();

            loop {
                if is_cancelled() || fatal_failure_c.load(Ordering::SeqCst) {
                    break;
                }

                let next_item = {
                    let mut lock = queue_c.lock().await;
                    lock.pop_front()
                };

                let (idx, table_item) = match next_item {
                    Some(it) => it,
                    None => break, // All tables in queue finished
                };

                let table_name = &table_item.name;

                // INSTANT FAST-SKIP: Pre-checked table with 0 changes
                if pre_skipped_c.contains(table_name) {
                    completed_table_names_c
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .insert(table_name.clone());
                    skipped_table_names_c
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .insert(table_name.clone());
                    let comp = completed_tables_c.fetch_add(1, Ordering::SeqCst) + 1;
                    emit_log(
                        &app_c,
                        "info",
                        format!("[Tabel '{}'] Dilewati: terverifikasi tidak ada data baru (0 changes).", table_name),
                    );
                    let c_names = completed_table_names_c.lock().unwrap_or_else(|p| p.into_inner()).clone();
                    let s_names = skipped_table_names_c.lock().unwrap_or_else(|p| p.into_inner()).clone();
                    emit_progress(
                        &app_c,
                        idx + 1,
                        total_tables_c,
                        comp,
                        &c_names,
                        &s_names,
                        table_name,
                        0,
                        total_rows_imported_c.load(Ordering::Relaxed),
                        "skipped",
                        started_at_c,
                        &failures_c,
                    );
                    continue;
                }

                let safe_table = match sanitize_identifier(table_name) {
                    Ok(s) => s,
                    Err(e) => {
                        record_failure(
                            &app_c,
                            &failures_c,
                            Some(table_name),
                            "Memvalidasi nama tabel",
                            e,
                        );
                        completed_table_names_c
                            .lock()
                            .unwrap_or_else(|p| p.into_inner())
                            .insert(table_name.clone());
                        completed_tables_c.fetch_add(1, Ordering::SeqCst);
                        continue;
                    }
                };

                let mut table_is_fresh = is_fresh;

                if !is_fresh {
                    let exists_locally = local_tables_c.contains(table_name);
                    if !exists_locally {
                        table_is_fresh = true;
                        emit_log(
                            &app_c,
                            "info",
                            format!(
                                "[Tabel '{}'] ℹ️ Tabel baru terdeteksi di server (belum ada di lokal). Mengalihkan ke Fresh Sync...",
                                table_name
                            ),
                        );
                    } else {
                        let remote_cols = remote_schema_c.get(table_name);
                        let local_cols = local_schema_c.get(table_name);

                        let schema_drift = match (remote_cols, local_cols) {
                            (Some(r_cols), Some(l_cols)) => {
                                r_cols.len() != l_cols.len()
                                    || r_cols.iter().zip(l_cols.iter()).any(|(r, l)| {
                                        r.0 != l.0
                                            || r.1.to_lowercase() != l.1.to_lowercase()
                                            || r.2 != l.2
                                    })
                            }
                            _ => true,
                        };

                        if schema_drift {
                            table_is_fresh = true;
                            schema_drift_tables_c
                                .lock()
                                .unwrap_or_else(|p| p.into_inner())
                                .insert(table_name.clone());
                            let payload = LogPayload {
                                r#type: "warning".to_string(),
                                message: format!(
                                    "[Tabel '{}'] ⚠️ Terdeteksi perubahan struktur tabel di server (skema lokal tidak selaras). Otomatis dialihkan ke FRESH SYNC untuk menyelaraskan struktur & data!",
                                    table_name
                                ),
                                timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
                                details: Some("Perubahan struktur terdeteksi di server, tabel dialihkan ke fresh sync".to_string()),
                                table_name: Some(table_name.clone()),
                            };
                            let _ = app_c.emit("sync-log", payload);
                        }
                    }
                }

                // A. Structure DDL
                let need_structure_ddl = table_item.sync_structure && (is_fresh || table_is_fresh || !local_tables_c.contains(table_name));
                if need_structure_ddl {
                    touched_tables.insert(table_name.clone());
                    emit_log(
                        &app_c,
                        "info",
                        format!(
                            "[{}/{}] 🏗️ [Worker {}] Mengambil DDL tabel '{}'...",
                            idx + 1,
                            total_tables_c,
                            worker_id + 1,
                            table_name
                        ),
                    );

                    let q = format!("SHOW CREATE TABLE {}", safe_table);
                    match sqlx::query(&q).fetch_one(&*pool_c).await {
                        Ok(row) => {
                            let ddl: String = row.try_get(1).unwrap_or_default();
                            if !ddl.is_empty() {
                                let ddl_sql = if table_is_fresh {
                                    format!(
                                        "SET FOREIGN_KEY_CHECKS=0;\nDROP TABLE IF EXISTS {};\n{};\n",
                                        safe_table, ddl
                                    )
                                } else {
                                    let safe_create = if !ddl
                                        .to_uppercase()
                                        .contains("CREATE TABLE IF NOT EXISTS")
                                    {
                                        ddl.replacen(
                                            "CREATE TABLE",
                                            "CREATE TABLE IF NOT EXISTS",
                                            1,
                                        )
                                    } else {
                                        ddl
                                    };
                                    format!(
                                        "SET FOREIGN_KEY_CHECKS=0;\n{};\n",
                                        safe_create
                                    )
                                };

                                if let Err(e) = local_stdin.write_all(ddl_sql.as_bytes()).await {
                                    fatal_failure_c.store(true, Ordering::SeqCst);
                                    record_failure(
                                        &app_c,
                                        &failures_c,
                                        Some(table_name),
                                        "Mengirim struktur ke MySQL lokal",
                                        e.to_string(),
                                    );
                                    break;
                                }
                                if let Err(e) = local_stdin.flush().await {
                                    fatal_failure_c.store(true, Ordering::SeqCst);
                                    record_failure(
                                        &app_c,
                                        &failures_c,
                                        Some(table_name),
                                        "Flush struktur ke MySQL lokal",
                                        e.to_string(),
                                    );
                                    break;
                                }
                                if !check_importer_running(
                                    &mut local_child,
                                    &app_c,
                                    &failures_c,
                                    &fatal_failure_c,
                                    table_name,
                                ) {
                                    break;
                                }
                                emit_log(
                                    &app_c,
                                    "info",
                                    format!(
                                        "[Tabel '{}'] Struktur dikirim ke MySQL lokal.",
                                        table_name
                                    ),
                                );
                            }
                        }
                        Err(e) => {
                            record_failure(
                                &app_c,
                                &failures_c,
                                Some(table_name),
                                "Mengambil struktur tabel remote",
                                e.to_string(),
                            );
                        }
                    }
                }

                // B. Data Sync (Direct Streaming into Worker's Local MySQL Process)
                let mut total_table_rows = 0usize;
                if table_item.sync_data {
                    touched_tables.insert(table_name.clone());
                    let pk_col_opt = pk_map_c.get(table_name).cloned();
                    let watermark = if !table_is_fresh {
                        watermarks_c
                            .as_ref()
                            .as_ref()
                            .and_then(|w| w.get(table_name))
                    } else {
                        None
                    };

                    let (where_clause, sync_desc) = if let Some(ref pk_col) = pk_col_opt {
                        let watermark_id = watermark.and_then(|wm| {
                            (wm.primary_key.eq_ignore_ascii_case(pk_col.as_str()))
                                .then(|| wm.last_id.parse::<i128>().ok())
                                .flatten()
                                .filter(|id| *id > 0)
                        });
                        let safe_pk =
                            sanitize_identifier(pk_col).unwrap_or_else(|_| format!("`{}`", pk_col));
                        let updated_at_col = updated_at_tables_c
                            .contains(table_name)
                            .then(|| "`updated_at`".to_string());
                        let watermark_timestamp = watermark
                            .and_then(|wm| wm.last_sync_time.as_deref())
                            .and_then(normalize_mysql_timestamp);
                        let update_filter = watermark_id
                            .zip(updated_at_col.as_deref())
                            .zip(watermark_timestamp.as_deref())
                            .map(|((id, column), timestamp)| (id, column, timestamp));

                        // FAST-CHECK: If incremental with watermark, check if rows actually exist before mysqldump
                        if !table_is_fresh {
                            let filter_cond = match (update_filter, watermark_id) {
                                (Some((w_id, col, ts)), _) => {
                                    Some(format!("{} > {} OR {} >= '{}'", safe_pk, w_id, col, ts))
                                }
                                (None, Some(w_id)) => Some(format!("{} > {}", safe_pk, w_id)),
                                _ => None,
                            };

                            if let Some(ref cond) = filter_cond {
                                let check_sql = format!("SELECT 1 FROM {} WHERE ({}) LIMIT 1", safe_table, cond);
                                let has_changes = match sqlx::query(&check_sql).fetch_optional(&*pool_c).await {
                                    Ok(Some(_)) => true,
                                    Ok(None) => false,
                                    Err(_) => true,
                                };

                                if !has_changes {
                                    completed_table_names_c
                                        .lock()
                                        .unwrap_or_else(|p| p.into_inner())
                                        .insert(table_name.clone());
                                    skipped_table_names_c
                                        .lock()
                                        .unwrap_or_else(|p| p.into_inner())
                                        .insert(table_name.clone());
                                    let comp = completed_tables_c.fetch_add(1, Ordering::SeqCst) + 1;
                                    emit_log(
                                        &app_c,
                                        "info",
                                        format!("[Tabel '{}'] Dilewati: tidak ada baris baru/perubahan.", table_name),
                                    );
                                    let c_names = completed_table_names_c.lock().unwrap_or_else(|p| p.into_inner()).clone();
                                    let s_names = skipped_table_names_c.lock().unwrap_or_else(|p| p.into_inner()).clone();
                                    emit_progress(
                                        &app_c,
                                        idx + 1,
                                        total_tables_c,
                                        comp,
                                        &c_names,
                                        &s_names,
                                        table_name,
                                        0,
                                        total_rows_imported_c.load(Ordering::Relaxed),
                                        "skipped",
                                        started_at_c,
                                        &failures_c,
                                    );
                                    continue;
                                }
                            } else if watermark.is_none() {
                                let check_sql = format!("SELECT 1 FROM {} LIMIT 1", safe_table);
                                if let Ok(None) = sqlx::query(&check_sql).fetch_optional(&*pool_c).await {
                                    completed_table_names_c
                                        .lock()
                                        .unwrap_or_else(|p| p.into_inner())
                                        .insert(table_name.clone());
                                    skipped_table_names_c
                                        .lock()
                                        .unwrap_or_else(|p| p.into_inner())
                                        .insert(table_name.clone());
                                    let comp = completed_tables_c.fetch_add(1, Ordering::SeqCst) + 1;
                                    emit_log(
                                        &app_c,
                                        "info",
                                        format!("[Tabel '{}'] Dilewati: tidak ada baris baru/perubahan.", table_name),
                                    );
                                    let c_names = completed_table_names_c.lock().unwrap_or_else(|p| p.into_inner()).clone();
                                    let s_names = skipped_table_names_c.lock().unwrap_or_else(|p| p.into_inner()).clone();
                                    emit_progress(
                                        &app_c,
                                        idx + 1,
                                        total_tables_c,
                                        comp,
                                        &c_names,
                                        &s_names,
                                        table_name,
                                        0,
                                        total_rows_imported_c.load(Ordering::Relaxed),
                                        "skipped",
                                        started_at_c,
                                        &failures_c,
                                    );
                                    continue;
                                }
                            }
                        }

                        match (update_filter, watermark_id, row_limit_opt) {
                            (Some((w_id, col, ts)), _, Some(lim)) => (
                                format!(
                                    "--where=({} > {} OR {} >= '{}') LIMIT {}",
                                    safe_pk, w_id, col, ts, lim
                                ),
                                format!(
                                    "Sync baris baru dan perubahan '{}' sejak {} (PK: {})...",
                                    table_name, ts, pk_col
                                ),
                            ),
                            (Some((w_id, col, ts)), _, None) => (
                                format!(
                                    "--where=({} > {} OR {} >= '{}')",
                                    safe_pk, w_id, col, ts
                                ),
                                format!(
                                    "Sync baris baru dan perubahan '{}' sejak {} (PK: {})...",
                                    table_name, ts, pk_col
                                ),
                            ),
                            (None, Some(w_id), Some(lim)) => (
                                format!("--where={} > {} LIMIT {}", safe_pk, w_id, lim),
                                format!(
                                    "Sync New & Update '{}' setelah ID {} (PK: {})...",
                                    table_name, w_id, pk_col
                                ),
                            ),
                            (None, Some(w_id), None) => (
                                format!("--where={} > {}", safe_pk, w_id),
                                format!(
                                    "Sync New & Update '{}' setelah ID {} (PK: {})...",
                                    table_name, w_id, pk_col
                                ),
                            ),
                            (None, None, Some(lim)) => (
                                format!("--where=1 LIMIT {}", lim),
                                format!("Fresh Sync tabel '{}' (limit: {})...", table_name, lim),
                            ),
                            (None, None, None) => (
                                String::new(),
                                format!("Fresh Sync tabel '{}'...", table_name),
                            ),
                        }
                    } else {
                        if !table_is_fresh {
                            let check_sql = format!("SELECT 1 FROM {} LIMIT 1", safe_table);
                            if let Ok(None) = sqlx::query(&check_sql).fetch_optional(&*pool_c).await {
                                completed_table_names_c
                                    .lock()
                                    .unwrap_or_else(|p| p.into_inner())
                                    .insert(table_name.clone());
                                skipped_table_names_c
                                    .lock()
                                    .unwrap_or_else(|p| p.into_inner())
                                    .insert(table_name.clone());
                                let comp = completed_tables_c.fetch_add(1, Ordering::SeqCst) + 1;
                                emit_log(
                                    &app_c,
                                    "info",
                                    format!("[Tabel '{}'] Dilewati: tidak ada data di server.", table_name),
                                );
                                let c_names = completed_table_names_c.lock().unwrap_or_else(|p| p.into_inner()).clone();
                                let s_names = skipped_table_names_c.lock().unwrap_or_else(|p| p.into_inner()).clone();
                                emit_progress(
                                    &app_c,
                                    idx + 1,
                                    total_tables_c,
                                    comp,
                                    &c_names,
                                    &s_names,
                                    table_name,
                                    0,
                                    total_rows_imported_c.load(Ordering::Relaxed),
                                    "skipped",
                                    started_at_c,
                                    &failures_c,
                                );
                                continue;
                            }
                        }

                        match row_limit_opt {
                            Some(lim) if lim > 0 => (
                                format!("--where=1 LIMIT {}", lim),
                                format!("Sync tabel '{}' (limit: {})...", table_name, lim),
                            ),
                            _ => (
                                String::new(),
                                format!("Sync tabel '{}'...", table_name),
                            ),
                        }
                    };

                    emit_log(
                        &app_c,
                        "info",
                        format!(
                            "[{}/{}] ⚡ [Worker {}] {}",
                            idx + 1,
                            total_tables_c,
                            worker_id + 1,
                            sync_desc
                        ),
                    );

                    match stream_dump_to_local_importer(
                        &app_c,
                        tunnel_port,
                        &remote_cfg_c,
                        table_name,
                        idx + 1,
                        &where_clause,
                        !table_is_fresh,
                        &mut local_stdin,
                        &total_rows_imported_c,
                        total_tables_c,
                        &completed_tables_c,
                        &completed_table_names_c,
                        &skipped_table_names_c,
                        started_at_c,
                        &failures_c,
                    )
                    .await
                    {
                        Ok(rows_streamed) => {
                            total_table_rows = rows_streamed;
                            if rows_streamed > 0 {
                                emit_log(
                                    &app_c,
                                    "info",
                                    format!(
                                        "[Tabel '{}'] 📦 Data dikirim (~{} baris)...",
                                        table_name,
                                        format_number(rows_streamed)
                                    ),
                                );
                            }
                        }
                        Err(e) => {
                            if !is_cancelled() {
                                record_failure(
                                    &app_c,
                                    &failures_c,
                                    Some(table_name),
                                    "Dump & Impor data tabel",
                                    e,
                                );
                            }
                        }
                    }

                    if !check_importer_running(
                        &mut local_child,
                        &app_c,
                        &failures_c,
                        &fatal_failure_c,
                        table_name,
                    ) {
                        break;
                    }
                }

                let comp = completed_tables_c.fetch_add(1, Ordering::SeqCst) + 1;
                completed_table_names_c
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .insert(table_name.clone());

                let has_fail = table_has_failure(&failures_c, table_name);
                let no_data_rows = table_has_no_data_to_sync(
                    table_item.sync_data,
                    total_table_rows,
                    has_fail,
                );

                if !table_item.sync_data {
                    if !need_structure_ddl {
                        skipped_table_names_c
                            .lock()
                            .unwrap_or_else(|p| p.into_inner())
                            .insert(table_name.clone());
                    }
                    if !has_fail {
                        emit_log(
                            &app_c,
                            "info",
                            if need_structure_ddl {
                                format!(
                                    "[Tabel '{}'] Selesai: struktur tabel disinkronkan (opsi data dinonaktifkan).",
                                    table_name
                                )
                            } else {
                                format!(
                                    "[Tabel '{}'] Dilewati: sinkronisasi data dinonaktifkan.",
                                    table_name
                                )
                            },
                        );
                    }
                } else if no_data_rows {
                    skipped_table_names_c
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .insert(table_name.clone());
                    emit_log(
                        &app_c,
                        "info",
                        format!(
                            "[Tabel '{}'] Dilewati: tidak ada baris baru/perubahan untuk dikirim.",
                            table_name
                        ),
                    );
                } else if !has_fail {
                    emit_log(
                        &app_c,
                        "info",
                        format!(
                            "[Tabel '{}'] Tahap pengiriman selesai: ~{} row dikirim ke MySQL lokal.",
                            table_name,
                            format_number(total_table_rows)
                        ),
                    );
                }

                let c_names = completed_table_names_c.lock().unwrap_or_else(|p| p.into_inner()).clone();
                let s_names = skipped_table_names_c.lock().unwrap_or_else(|p| p.into_inner()).clone();
                emit_progress(
                    &app_c,
                    idx + 1,
                    total_tables_c,
                    comp,
                    &c_names,
                    &s_names,
                    table_name,
                    total_table_rows,
                    total_rows_imported_c.load(Ordering::Relaxed),
                    if (!table_item.sync_data && !need_structure_ddl) || no_data_rows {
                        "skipped"
                    } else {
                        "completed"
                    },
                    started_at_c,
                    &failures_c,
                );
            }

            // Postlude cleanup
            let postlude = "\nCOMMIT;\nSET FOREIGN_KEY_CHECKS=1;\nSET UNIQUE_CHECKS=1;\nSET AUTOCOMMIT=1;\n";
            if !is_cancelled() {
                let _ = local_stdin.write_all(postlude.as_bytes()).await;
                let _ = local_stdin.flush().await;
            }
            drop(local_stdin);

            let status = match local_child.wait().await {
                Ok(s) => s,
                Err(e) => {
                    if let Some(pid) = local_pid {
                        unregister_child_pid(pid);
                    }
                    fatal_failure_c.store(true, Ordering::SeqCst);
                    return Err(format!("Gagal menunggu proses MySQL lokal: {}", e));
                }
            };
            if let Some(pid) = local_pid {
                unregister_child_pid(pid);
            }

            let stderr_output = match stderr_task.await {
                Ok(Ok(s)) => s,
                _ => String::new(),
            };
            if !is_cancelled() && (!status.success() || has_mysql_cli_errors(&stderr_output)) {
                let message = if stderr_output.is_empty() {
                    format!("Proses MySQL lokal berhenti dengan status {}", status)
                } else {
                    let clean = extract_clean_mysql_error(&stderr_output);
                    if is_schema_mismatch_error(&stderr_output) {
                        format!("Ketidakcocokan skema tabel: {}", clean)
                    } else {
                        clean
                    }
                };
                if touched_tables.is_empty() {
                    record_failure(
                        &app_c,
                        &failures_c,
                        None,
                        "Importer MySQL lokal",
                        message,
                    );
                } else {
                    for t in touched_tables {
                        record_failure(
                            &app_c,
                            &failures_c,
                            Some(&t),
                            "Importer MySQL lokal",
                            message.clone(),
                        );
                    }
                }
            }

            Ok(())
        });
        worker_handles.push(handle);
    }

    // Wait for all workers to finish
    for h in worker_handles {
        if let Err(e) = h.await {
            record_failure(
                &app,
                &failures,
                None,
                "Worker pipeline sync",
                e.to_string(),
            );
        }
    }

    let completed_table_count = completed_tables.load(Ordering::SeqCst);
    let total_rows_all_tables = total_rows_imported.load(Ordering::SeqCst);
    let completed_table_names = completed_table_names
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone();
    let skipped_table_names = skipped_table_names
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone();

    let _ = remote_pool.close().await;
    local_schema_pool.close().await;
    drop(tunnel);

    let duration = start_time.elapsed();
    let was_cancelled = is_cancelled();
    if !was_cancelled && completed_table_count != total_tables {
        record_failure(
            &app,
            &failures,
            None,
            "Sinkronisasi",
            "Proses berhenti sebelum seluruh tabel selesai",
        );
    }
    let failures = failures
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone();
    let failure_count = failures.len();
    let all_tables_processed = completed_table_count == total_tables;
    let status = resolve_sync_status(
        was_cancelled,
        fatal_failure.load(Ordering::SeqCst),
        all_tables_processed,
        !failures.is_empty(),
    );
    let (processed_tables, successful_tables, failed_tables, skipped_tables) =
        progress_table_counts(&completed_table_names, &skipped_table_names, &failures);

    emit_progress(
        &app,
        completed_table_count,
        total_tables,
        completed_table_count,
        &completed_table_names,
        &skipped_table_names,
        "",
        0,
        total_rows_all_tables,
        status,
        start_time,
        &failures_consumer,
    );

    let schema_drift_tables_list: Vec<String> = schema_drift_tables
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .iter()
        .cloned()
        .collect();

    let result = SyncResult {
        success: status == "success",
        status: status.to_string(),
        total_tables: completed_table_count,
        total_rows_synced: total_rows_all_tables,
        duration_ms: duration.as_millis() as u64,
        processed_tables,
        successful_tables,
        failed_tables,
        skipped_tables,
        sync_timestamp: Some(sync_timestamp),
        sync_timestamp_epoch_ms: Some(sync_timestamp_epoch_ms),
        error: match status {
            "success" => None,
            "partial" => Some(format!(
                "Sinkronisasi selesai dengan {} masalah",
                failure_count
            )),
            "cancelled" => Some("Dibatalkan oleh pengguna".to_string()),
            _ => Some("Sinkronisasi berhenti sebelum semua tabel selesai".to_string()),
        },
        failures,
        schema_drift_tables: schema_drift_tables_list,
    };

    let _ = app.emit("sync-finished", &result);

    use tauri::Manager;
    for (_, win) in app.webview_windows() {
        let _ = win.set_progress_bar(tauri::window::ProgressBarState {
            progress: None,
            status: Some(tauri::window::ProgressBarStatus::None),
        });
        let finish_title = match status {
            "success" => "[✓ Selesai] DB Sync Engine SSH",
            "partial" => "[⚠ Sebagian Selesai] DB Sync Engine SSH",
            "cancelled" => "[Dibatalkan] DB Sync Engine SSH",
            _ => "[✕ Gagal] DB Sync Engine SSH",
        };
        let _ = win.set_title(finish_title);
    }
    let (log_type, summary) = match status {
        "success" => (
            "success",
            format!(
                "🎉 Sinkronisasi berhasil: {} tabel diproses, total {} row dikirim dalam {:.1} detik.",
                completed_table_count,
                format_number(total_rows_all_tables),
                duration.as_secs_f64()
            ),
        ),
        "partial" => (
            "warning",
            format!(
                "⚠️ Sinkronisasi selesai sebagian: {} dari {} tabel diproses; {} masalah ditemukan dalam {:.1} detik.",
                completed_table_count,
                total_tables,
                failure_count,
                duration.as_secs_f64()
            ),
        ),
        "cancelled" => (
            "warning",
            format!(
                "⏹ Sinkronisasi dibatalkan: {} dari {} tabel diproses, {} row dikirim ({:.1}s).",
                completed_table_count,
                total_tables,
                format_number(total_rows_all_tables),
                duration.as_secs_f64()
            ),
        ),
        _ => (
            "error",
            format!(
                "❌ Sinkronisasi gagal sebelum semua tabel selesai: {} dari {} tabel diproses; {} masalah ditemukan ({:.1}s).",
                completed_table_count,
                total_tables,
                failure_count,
                duration.as_secs_f64()
            ),
        ),
    };
    emit_log(&app, log_type, summary);

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::{
        build_chunk_where_clause, extract_clean_mysql_error, has_mysql_cli_errors,
        is_schema_mismatch_error, normalize_mysql_timestamp, progress_table_counts,
        resolve_sync_status, table_has_no_data_to_sync, MysqlDumpRowCounter,
    };
    use crate::models::SyncFailure;
    use std::collections::HashSet;

    #[test]
    fn mysql_dump_row_counter_handles_newlines_chunks_and_quoted_delimiters() {
        let mut counter = MysqlDumpRowCounter::default();

        counter.push(b"INSERT INTO `items` VALUES\n(1, 'contains ),( inside'),\n");
        counter.push(b"(2, 'another row'); REPLACE INTO `items` VALUES (3, 'three');");

        assert_eq!(counter.total_rows, 3);
    }

    #[test]
    fn empty_incremental_table_is_skipped_but_failure_or_disabled_data_is_not() {
        assert!(table_has_no_data_to_sync(true, 0, false));
        assert!(!table_has_no_data_to_sync(true, 5, false));
        assert!(!table_has_no_data_to_sync(true, 0, true));
        assert!(!table_has_no_data_to_sync(false, 0, false));
    }

    #[test]
    fn progress_counts_unique_failed_and_skipped_tables() {
        let completed = ["customers", "orders", "empty_table"]
            .into_iter()
            .map(str::to_string)
            .collect();
        let skipped = ["empty_table"].into_iter().map(str::to_string).collect();
        let failures = vec![
            SyncFailure {
                table_name: Some("orders".to_string()),
                stage: "Dump SSH".to_string(),
                message: "connection lost".to_string(),
            },
            SyncFailure {
                table_name: Some("orders".to_string()),
                stage: "Importer MySQL lokal".to_string(),
                message: "import failed".to_string(),
            },
            SyncFailure {
                table_name: None,
                stage: "SSH".to_string(),
                message: "connection lost".to_string(),
            },
        ];

        assert_eq!(
            progress_table_counts(&completed, &skipped, &failures),
            (3, 1, 1, 1)
        );
        assert_eq!(
            progress_table_counts(&HashSet::new(), &HashSet::new(), &failures),
            (1, 0, 1, 0)
        );
    }

    #[test]
    fn mysql_cli_errors_are_detected_without_treating_warnings_as_failures() {
        assert!(has_mysql_cli_errors(
            "ERROR 1054 (42S22) at line 10: Unknown column"
        ));
        assert!(has_mysql_cli_errors(" ERROR: connection lost"));
        assert!(!has_mysql_cli_errors("Warning: using a password on the command line"));
        assert!(!has_mysql_cli_errors(""));
    }

    #[test]
    fn sync_status_is_success_only_when_everything_finishes_cleanly() {
        assert_eq!(resolve_sync_status(false, false, true, false), "success");
    }

    #[test]
    fn sync_status_is_partial_when_all_tables_finish_with_table_errors() {
        assert_eq!(resolve_sync_status(false, false, true, true), "partial");
    }

    #[test]
    fn sync_status_is_failed_when_pipeline_stops_before_all_tables() {
        assert_eq!(resolve_sync_status(false, false, false, true), "failed");
    }

    #[test]
    fn sync_status_is_failed_for_a_fatal_importer_error() {
        assert_eq!(resolve_sync_status(false, true, true, true), "failed");
    }

    #[test]
    fn sync_status_keeps_cancellation_distinct_from_failure() {
        assert_eq!(resolve_sync_status(true, true, false, true), "cancelled");
    }

    #[test]
    fn mysql_watermark_timestamp_accepts_remote_datetime_format() {
        assert_eq!(
            normalize_mysql_timestamp("2026-06-02 13:14:15.123456").as_deref(),
            Some("2026-06-02 13:14:15.123456")
        );
    }

    #[test]
    fn mysql_watermark_timestamp_rejects_local_iso_timestamp() {
        assert_eq!(normalize_mysql_timestamp("2026-06-02T13:14:15.123Z"), None);
    }

    #[test]
    fn incremental_chunk_filter_includes_updated_rows_below_id_watermark() {
        let clause = build_chunk_where_clause(
            "`id`",
            None,
            500,
            Some((900, "`updated_at`", "2026-06-02 13:14:15.123456")),
        );

        assert_eq!(
            clause,
            "--where=(`id` > 900 OR `updated_at` >= '2026-06-02 13:14:15.123456') AND `id` <= 500"
        );
    }

    #[test]
    fn incremental_chunk_filter_preserves_keyset_range_without_timestamp_watermark() {
        assert_eq!(
            build_chunk_where_clause("`id`", Some(100), 500, None),
            "--where=`id` > 100 AND `id` <= 500"
        );
    }

    #[test]
    fn first_chunk_includes_zero_and_negative_primary_keys() {
        assert_eq!(
            build_chunk_where_clause("`id`", None, 500, None),
            "--where=`id` <= 500"
        );
    }

    #[test]
    fn schema_mismatch_detection_identifies_structure_errors_and_extracts_clean_line() {
        let err1 = "ERROR 1054 (42S22) at line 12: Unknown column 'new_col' in 'field list'";
        assert!(is_schema_mismatch_error(err1));
        assert_eq!(extract_clean_mysql_error(err1), err1);

        let err2 = "ERROR 1146 (42S02): Table 'test.not_exists' doesn't exist";
        assert!(is_schema_mismatch_error(err2));
        assert_eq!(extract_clean_mysql_error(err2), err2);

        let generic_err = "ERROR 2006 (HY000): MySQL server has gone away";
        assert!(!is_schema_mismatch_error(generic_err));
        assert_eq!(extract_clean_mysql_error(generic_err), generic_err);
    }
}
