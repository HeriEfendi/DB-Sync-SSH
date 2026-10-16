use crate::local_db::{ensure_local_database_exists, sanitize_identifier, spawn_mysql_child};
use crate::models::{
    incremental_primary_key, LocalDbConfig, LogPayload, ProgressPayload, RemoteDbConfig, SshConfig,
    SyncFailure, SyncOptions, SyncResult, TableInfo,
};
use crate::ssh_client::{establish_ssh_session, start_ssh_tunnel};
use sqlx::mysql::MySqlPoolOptions;
use sqlx::Row;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
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
    current_table_name: &str,
    rows_synced_current_table: usize,
    total_synced_all_tables: usize,
    status: &str,
) {
    let payload = ProgressPayload {
        current_table_index,
        total_tables,
        current_table_name: current_table_name.to_string(),
        rows_synced_current_table,
        total_synced_all_tables,
        status: status.to_string(),
    };
    let _ = app.emit("sync-progress", payload);
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
        remote_db_config.database
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
        remote_db_config.database
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

#[derive(Debug)]
pub enum PipelineMsg {
    Ddl {
        table_name: String,
        table_index: usize,
        sql: String,
    },
    DataChunk {
        table_name: String,
        table_index: usize,
        chunk_num: usize,
        sql_bytes: Vec<u8>,
        row_count: usize,
    },
    TableFinished {
        table_name: String,
        table_index: usize,
        total_rows_synced: usize,
    },
}

async fn dump_table_chunk_to_bytes(
    tunnel_port: u16,
    remote_db_config: &RemoteDbConfig,
    table_name: &str,
    where_clause: &str,
    use_replace: bool,
) -> Result<(Vec<u8>, usize), String> {
    let mut dump_cmd = crate::process_utils::new_tokio_command("mysqldump");
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
        .arg("--single-transaction")
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
        .map_err(|e| format!("Gagal menjalankan mysqldump: {}", e))?;

    let dump_pid = dump_child.id();
    if let Some(pid) = dump_pid {
        register_child_pid(pid);
    }

    let mut output_bytes = Vec::new();
    let mut rows_count = 0usize;
    let mut stderr = dump_child
        .stderr
        .take()
        .ok_or_else(|| "Gagal membaca STDERR proses mysqldump".to_string())?;
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
        use tokio::io::AsyncReadExt;
        let mut buffer = [0u8; 65536];
        loop {
            if is_cancelled() {
                let _ = dump_child.kill().await;
                break;
            }
            match stdout.read(&mut buffer).await {
                Ok(0) => break,
                Ok(n) => {
                    let chunk = &buffer[..n];
                    let chunk_str = String::from_utf8_lossy(chunk);
                    for line in chunk_str.lines() {
                        if line.starts_with("INSERT INTO") || line.starts_with("REPLACE INTO") {
                            rows_count += line.matches("),(").count() + 1;
                        }
                    }
                    output_bytes.extend_from_slice(chunk);
                }
                Err(e) => {
                    let _ = dump_child.kill().await;
                    let _ = dump_child.wait().await;
                    if let Some(pid) = dump_pid {
                        unregister_child_pid(pid);
                    }
                    let _ = stderr_task.await;
                    return Err(format!("Gagal membaca stream mysqldump: {}", e));
                }
            }
        }
    }

    let status = match dump_child.wait().await {
        Ok(status) => status,
        Err(e) => {
            if let Some(pid) = dump_pid {
                unregister_child_pid(pid);
            }
            return Err(format!("Gagal menunggu proses mysqldump: {}", e));
        }
    };
    if let Some(pid) = dump_pid {
        unregister_child_pid(pid);
    }
    let stderr_output = stderr_task
        .await
        .map_err(|e| format!("Gagal mengambil keluaran error mysqldump: {}", e))?
        .map_err(|e| format!("Gagal membaca STDERR mysqldump: {}", e))?;

    if !is_cancelled() && !status.success() {
        let message = if stderr_output.is_empty() {
            format!("mysqldump berhenti dengan status {}", status)
        } else {
            stderr_output
        };
        return Err(message);
    }

    Ok((output_bytes, rows_count))
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

    // Pre-query primary keys and update timestamp columns in batch queries.
    let pk_query = "
        SELECT TABLE_NAME, COLUMN_NAME, DATA_TYPE, COLUMN_TYPE
        FROM information_schema.COLUMNS 
        WHERE TABLE_SCHEMA = DATABASE() AND COLUMN_KEY = 'PRI'
        ORDER BY ORDINAL_POSITION ASC
    ";
    let mut pk_columns: HashMap<String, Vec<(String, String, String)>> = HashMap::new();
    let rows = sqlx::query(pk_query)
        .fetch_all(&*remote_pool)
        .await
        .map_err(|e| format!("Gagal membaca metadata primary key remote: {}", e))?;
    for r in rows {
        let t_name: String = r.try_get("TABLE_NAME").unwrap_or_default();
        let c_name: String = r.try_get("COLUMN_NAME").unwrap_or_default();
        let data_type: String = r.try_get("DATA_TYPE").unwrap_or_default();
        let column_type: String = r.try_get("COLUMN_TYPE").unwrap_or_default();
        if !t_name.is_empty() && !c_name.is_empty() {
            pk_columns
                .entry(t_name)
                .or_default()
                .push((c_name, data_type, column_type));
        }
    }
    let pk_map: HashMap<String, String> = pk_columns
        .iter()
        .filter_map(|(table, columns)| {
            incremental_primary_key(columns).map(|column| (table.clone(), column))
        })
        .collect();

    let updated_at_query = "
        SELECT TABLE_NAME
        FROM information_schema.COLUMNS
        WHERE TABLE_SCHEMA = DATABASE() AND LOWER(COLUMN_NAME) = 'updated_at'
    ";
    let updated_at_rows = sqlx::query(updated_at_query)
        .fetch_all(&*remote_pool)
        .await
        .map_err(|e| format!("Gagal membaca metadata kolom updated_at remote: {}", e))?;
    let tables_with_updated_at: HashSet<String> = updated_at_rows
        .iter()
        .filter_map(|row| row.try_get::<String, _>("TABLE_NAME").ok())
        .collect();
    let pk_map: HashMap<String, String> = pk_map
        .into_iter()
        .filter(|(table, _)| tables_with_updated_at.contains(table))
        .collect();
    let pk_map = Arc::new(pk_map);
    let tables_with_updated_at = Arc::new(tables_with_updated_at);

    let sync_timestamp: String =
        sqlx::query_scalar("SELECT DATE_FORMAT(CURRENT_TIMESTAMP(6), '%Y-%m-%d %H:%i:%s.%f')")
            .fetch_one(&*remote_pool)
            .await
            .map_err(|e| format!("Gagal membaca waktu server MySQL remote: {}", e))?;

    let total_tables = sync_options.tables.len();
    let sync_mode = sync_options.sync_mode.as_deref().unwrap_or("incremental");
    let is_fresh = sync_mode == "fresh";
    let download_concurrency = if total_tables >= 2 { 2 } else { 1 };

    emit_log(
        &app,
        "info",
        format!(
            "🚀 Memulai Pipeline Sync ({} tabel, {} Remote SSH Download Workers, 1 Local Import Worker, Mode: {})...",
            total_tables,
            download_concurrency,
            if is_fresh { "Fresh Sync" } else { "Sync New & Update" }
        ),
    );

    // Bounded channel: 4 chunks capacity to prevent excessive RAM use while saturating pipeline
    let (tx, mut rx) = tokio::sync::mpsc::channel::<PipelineMsg>(4);

    let tables_queue = Arc::new(tokio::sync::Mutex::new(
        sync_options
            .tables
            .into_iter()
            .enumerate()
            .collect::<VecDeque<_>>(),
    ));
    let incremental_watermarks = Arc::new(sync_options.incremental_watermarks);
    let remote_db_cfg = Arc::new(remote_db_config);
    let failures_consumer = Arc::new(Mutex::new(Vec::<SyncFailure>::new()));
    let failures = failures_consumer.clone();
    let fatal_failure = Arc::new(AtomicBool::new(false));

    // ----------------------------------------------------
    // Consumer Task: Local MySQL Importer Worker
    // ----------------------------------------------------
    let local_cfg_c = local_db_config.clone();
    let app_consumer = app.clone();
    let failures_c = failures_consumer.clone();
    let fatal_failure_c = fatal_failure.clone();
    let total_tables_c = total_tables;

    let consumer_handle = tokio::spawn(async move {
        let (mut local_child, mut local_stdin, mut local_stderr) =
            match spawn_mysql_child(&local_cfg_c) {
                Ok(c) => c,
                Err(e) => {
                    fatal_failure_c.store(true, Ordering::SeqCst);
                    record_failure(
                        &app_consumer,
                        &failures_c,
                        None,
                        "Menjalankan importer MySQL lokal",
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
        let prelude = "SET SESSION sql_log_bin=0;\nSET FOREIGN_KEY_CHECKS=0;\nSET UNIQUE_CHECKS=0;\nSET AUTOCOMMIT=1;\n";
        if let Err(e) = local_stdin.write_all(prelude.as_bytes()).await {
            fatal_failure_c.store(true, Ordering::SeqCst);
            record_failure(
                &app_consumer,
                &failures_c,
                None,
                "Menyiapkan sesi MySQL lokal",
                e.to_string(),
            );
        }

        let mut total_rows_imported = 0usize;
        let mut completed_tables = 0usize;
        let mut touched_tables = std::collections::HashSet::new();
        let mut stdin_failed = false;

        while let Some(msg) = rx.recv().await {
            if is_cancelled() {
                break;
            }
            match msg {
                PipelineMsg::Ddl {
                    table_name,
                    table_index: _,
                    sql,
                } => {
                    touched_tables.insert(table_name.clone());
                    if stdin_failed {
                        record_failure(
                            &app_consumer,
                            &failures_c,
                            Some(&table_name),
                            "Mengirim struktur ke MySQL lokal",
                            "Koneksi stdin importer sudah terputus; DDL tidak diterapkan",
                        );
                    } else if let Err(e) = local_stdin.write_all(sql.as_bytes()).await {
                        stdin_failed = true;
                        fatal_failure_c.store(true, Ordering::SeqCst);
                        record_failure(
                            &app_consumer,
                            &failures_c,
                            Some(&table_name),
                            "Mengirim struktur ke MySQL lokal",
                            e.to_string(),
                        );
                    } else {
                        if let Err(e) = local_stdin.flush().await {
                            stdin_failed = true;
                            fatal_failure_c.store(true, Ordering::SeqCst);
                            record_failure(
                                &app_consumer,
                                &failures_c,
                                Some(&table_name),
                                "Flush struktur ke MySQL lokal",
                                e.to_string(),
                            );
                        } else {
                            if !check_importer_running(
                                &mut local_child,
                                &app_consumer,
                                &failures_c,
                                &fatal_failure_c,
                                &table_name,
                            ) {
                                stdin_failed = true;
                            } else {
                                emit_log(
                                    &app_consumer,
                                    "info",
                                    format!(
                                        "[Tabel '{}'] Struktur dikirim ke MySQL lokal.",
                                        table_name
                                    ),
                                );
                            }
                        }
                    }
                }
                PipelineMsg::DataChunk {
                    table_name,
                    table_index,
                    chunk_num,
                    sql_bytes,
                    row_count,
                } => {
                    touched_tables.insert(table_name.clone());
                    let mut chunk_written = if sql_bytes.is_empty() {
                        true
                    } else if stdin_failed {
                        record_failure(
                            &app_consumer,
                            &failures_c,
                            Some(&table_name),
                            "Mengirim data ke MySQL lokal",
                            format!(
                                "Chunk {} tidak dikirim karena stdin importer terputus",
                                chunk_num
                            ),
                        );
                        false
                    } else if let Err(e) = local_stdin.write_all(&sql_bytes).await {
                        stdin_failed = true;
                        fatal_failure_c.store(true, Ordering::SeqCst);
                        record_failure(
                            &app_consumer,
                            &failures_c,
                            Some(&table_name),
                            "Mengirim data ke MySQL lokal",
                            format!("Chunk {}: {}", chunk_num, e),
                        );
                        false
                    } else if let Err(e) = local_stdin.flush().await {
                        stdin_failed = true;
                        fatal_failure_c.store(true, Ordering::SeqCst);
                        record_failure(
                            &app_consumer,
                            &failures_c,
                            Some(&table_name),
                            "Flush data ke MySQL lokal",
                            format!("Chunk {}: {}", chunk_num, e),
                        );
                        false
                    } else {
                        true
                    };

                    if chunk_written
                        && !sql_bytes.is_empty()
                        && !check_importer_running(
                            &mut local_child,
                            &app_consumer,
                            &failures_c,
                            &fatal_failure_c,
                            &table_name,
                        )
                    {
                        stdin_failed = true;
                        chunk_written = false;
                    }

                    if chunk_written {
                        total_rows_imported += row_count;
                        if row_count > 0 {
                            emit_log(
                                &app_consumer,
                                "info",
                                format!(
                                    "[Tabel '{}'] 📦 Chunk {} dikirim (~{} baris)...",
                                    table_name,
                                    chunk_num,
                                    format_number(row_count)
                                ),
                            );
                        }
                    }
                    emit_progress(
                        &app_consumer,
                        table_index,
                        total_tables_c,
                        &table_name,
                        if chunk_written { row_count } else { 0 },
                        total_rows_imported,
                        "syncing",
                    );
                }
                PipelineMsg::TableFinished {
                    table_name,
                    table_index,
                    total_rows_synced,
                } => {
                    completed_tables += 1;
                    if !table_has_failure(&failures_c, &table_name) {
                        emit_log(
                            &app_consumer,
                            "info",
                            format!(
                                "[Tabel '{}'] Tahap pengiriman selesai: ~{} row dikirim; menunggu verifikasi importer.",
                                table_name,
                                format_number(total_rows_synced)
                            ),
                        );
                    }
                    emit_progress(
                        &app_consumer,
                        table_index,
                        total_tables_c,
                        &table_name,
                        total_rows_synced,
                        total_rows_imported,
                        "completed",
                    );
                }
            }
        }

        let postlude = "\nSET FOREIGN_KEY_CHECKS=1;\nSET UNIQUE_CHECKS=1;\n";
        if !is_cancelled() {
            if let Err(e) = local_stdin.write_all(postlude.as_bytes()).await {
                fatal_failure_c.store(true, Ordering::SeqCst);
                record_failure(
                    &app_consumer,
                    &failures_c,
                    None,
                    "Menutup sesi MySQL lokal",
                    e.to_string(),
                );
            } else if let Err(e) = local_stdin.flush().await {
                fatal_failure_c.store(true, Ordering::SeqCst);
                record_failure(
                    &app_consumer,
                    &failures_c,
                    None,
                    "Flush akhir MySQL lokal",
                    e.to_string(),
                );
            }
        }
        drop(local_stdin);

        let status = match local_child.wait().await {
            Ok(status) => status,
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
        let stderr_output = stderr_task
            .await
            .map_err(|e| format!("Gagal mengambil keluaran error MySQL lokal: {}", e))?
            .map_err(|e| format!("Gagal membaca STDERR MySQL lokal: {}", e))?;

        if !is_cancelled() && !status.success() {
            let message = if stderr_output.is_empty() {
                format!("Proses MySQL lokal berhenti dengan status {}", status)
            } else {
                stderr_output
            };
            if touched_tables.is_empty() {
                record_failure(
                    &app_consumer,
                    &failures_c,
                    None,
                    "Importer MySQL lokal",
                    message,
                );
            } else {
                for table_name in touched_tables {
                    record_failure(
                        &app_consumer,
                        &failures_c,
                        Some(&table_name),
                        "Importer MySQL lokal",
                        message.clone(),
                    );
                }
            }
        }

        Ok((completed_tables, total_rows_imported))
    });

    // ----------------------------------------------------
    // Producer Tasks: 2 Download Workers over SSH Tunnel
    // ----------------------------------------------------
    let mut download_handles = Vec::new();
    const CHUNK_SIZE: usize = 50_000;

    for worker_id in 0..download_concurrency {
        let queue_c = tables_queue.clone();
        let pool_c = remote_pool.clone();
        let pk_map_c = pk_map.clone();
        let updated_at_tables_c = tables_with_updated_at.clone();
        let watermarks_c = incremental_watermarks.clone();
        let remote_cfg_c = remote_db_cfg.clone();
        let app_c = app.clone();
        let failures_p = failures.clone();
        let tx_c = tx.clone();
        let row_limit_opt = sync_options.row_limit;

        let handle = tokio::spawn(async move {
            loop {
                if is_cancelled() {
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
                let safe_table = match sanitize_identifier(table_name) {
                    Ok(s) => s,
                    Err(e) => {
                        record_failure(
                            &app_c,
                            &failures_p,
                            Some(table_name),
                            "Memvalidasi nama tabel",
                            e,
                        );
                        continue;
                    }
                };

                // A. Structure DDL
                if table_item.sync_structure {
                    emit_log(
                        &app_c,
                        "info",
                        format!(
                            "[{}/{}] 🏗️ [Worker {}] Mengambil DDL tabel '{}'...",
                            idx + 1,
                            total_tables,
                            worker_id + 1,
                            table_name
                        ),
                    );

                    let q = format!("SHOW CREATE TABLE {}", safe_table);
                    match sqlx::query(&q).fetch_one(&*pool_c).await {
                        Ok(row) => {
                            let ddl: String = match row.try_get(1) {
                                Ok(ddl) => ddl,
                                Err(e) => {
                                    record_failure(
                                        &app_c,
                                        &failures_p,
                                        Some(table_name),
                                        "Membaca struktur tabel remote",
                                        e.to_string(),
                                    );
                                    String::new()
                                }
                            };
                            if !ddl.is_empty() {
                                let ddl_sql = if is_fresh {
                                    format!(
                                    "SET FOREIGN_KEY_CHECKS=0;\nDROP TABLE IF EXISTS {};\n{};\nSET FOREIGN_KEY_CHECKS=1;\n",
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
                                    "SET FOREIGN_KEY_CHECKS=0;\n{};\nSET FOREIGN_KEY_CHECKS=1;\n",
                                    safe_create
                                )
                                };
                                if tx_c
                                    .send(PipelineMsg::Ddl {
                                        table_name: table_name.clone(),
                                        table_index: idx + 1,
                                        sql: ddl_sql,
                                    })
                                    .await
                                    .is_err()
                                {
                                    record_failure(
                                        &app_c,
                                        &failures_p,
                                        Some(table_name),
                                        "Mengirim struktur ke worker MySQL lokal",
                                        "Worker impor lokal sudah berhenti",
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            record_failure(
                                &app_c,
                                &failures_p,
                                Some(table_name),
                                "Mengambil struktur tabel remote",
                                e.to_string(),
                            );
                        }
                    }
                }

                // B. Data Sync (with Chunking for large tables)
                if table_item.sync_data {
                    let pk_col_opt = pk_map_c.get(table_name).cloned();
                    let watermark = if !is_fresh {
                        watermarks_c
                            .as_ref()
                            .as_ref()
                            .and_then(|w| w.get(table_name))
                    } else {
                        None
                    };

                    let mut total_table_rows = 0usize;

                    if let Some(ref pk_col) = pk_col_opt {
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
                        let use_updated_at_filter = update_filter.is_some();
                        let start_id = if watermark_id.is_some() && !use_updated_at_filter {
                            emit_log(
                                &app_c,
                                "warning",
                                format!(
                                    "[Tabel '{}'] Kolom updated_at tersedia, tetapi waktu watermark sebelumnya belum ada atau formatnya tidak dikenali; memakai full upsert agar perubahan baris lama tetap tertangkap.",
                                    table_name
                                ),
                            );
                            None
                        } else if use_updated_at_filter {
                            None
                        } else {
                            watermark_id
                        };
                        let mut current_last_id = start_id;
                        let mut chunk_num = 1;

                        if let Some((_, _, timestamp)) = update_filter {
                            emit_log(
                                &app_c,
                                "info",
                                format!(
                                    "[{}/{}] ⚡ [Worker {}] Sync baris baru dan perubahan '{}' sejak {} (PK: {})...",
                                    idx + 1,
                                    total_tables,
                                    worker_id + 1,
                                    table_name,
                                    timestamp,
                                    pk_col
                                ),
                            );
                        } else if let Some(start_id) = start_id {
                            emit_log(
                                &app_c,
                                "info",
                                format!(
                                    "[{}/{}] ⚡ [Worker {}] Sync New & Update '{}' setelah ID {} (PK: {})...",
                                    idx + 1,
                                    total_tables,
                                    worker_id + 1,
                                    table_name,
                                    start_id,
                                    pk_col
                                ),
                            );
                        } else if is_fresh {
                            emit_log(
                                &app_c,
                                "info",
                                format!(
                                    "[{}/{}] 🔄 [Worker {}] Fresh Sync tabel '{}'...",
                                    idx + 1,
                                    total_tables,
                                    worker_id + 1,
                                    table_name
                                ),
                            );
                        }

                        loop {
                            if is_cancelled() {
                                break;
                            }

                            let this_chunk_limit = match row_limit_opt {
                                Some(lim) => {
                                    if total_table_rows >= lim {
                                        break;
                                    }
                                    (lim - total_table_rows).min(CHUNK_SIZE)
                                }
                                None => CHUNK_SIZE,
                            };

                            // Fast index-only covering scan to find upper bound of this chunk
                            let bound_condition = current_last_id
                                .map(|last_id| format!("{} > {}", safe_pk, last_id))
                                .unwrap_or_else(|| "1=1".to_string());
                            let bound_q = format!(
                                "SELECT CAST(MAX(t.pk) AS CHAR), COUNT(*) FROM (SELECT {} AS pk FROM {} WHERE {} ORDER BY {} ASC LIMIT {}) AS t",
                                safe_pk, safe_table, bound_condition, safe_pk, this_chunk_limit
                            );

                            let (max_pk_in_chunk, keys_in_chunk): (Option<i128>, u64) =
                                match sqlx::query(&bound_q).fetch_one(&*pool_c).await {
                                    Ok(row) => {
                                        let max_pk = match row.try_get::<Option<String>, _>(0) {
                                            Ok(Some(value)) => match value.parse::<i128>() {
                                                Ok(value) => Some(value),
                                                Err(e) => {
                                                    record_failure(
                                                        &app_c,
                                                        &failures_p,
                                                        Some(table_name),
                                                        "Membaca batas primary key",
                                                        format!("Nilai primary key tidak valid '{}': {}", value, e),
                                                    );
                                                    break;
                                                }
                                            },
                                            Ok(None) => None,
                                            Err(e) => {
                                                record_failure(
                                                    &app_c,
                                                    &failures_p,
                                                    Some(table_name),
                                                    "Membaca batas primary key",
                                                    e.to_string(),
                                                );
                                                break;
                                            }
                                        };
                                        let key_count = match row.try_get::<i64, _>(1) {
                                            Ok(value) => {
                                                match u64::try_from(value) {
                                                    Ok(value) => value,
                                                    Err(e) => {
                                                        record_failure(
                                                        &app_c,
                                                        &failures_p,
                                                        Some(table_name),
                                                        "Membaca jumlah baris chunk",
                                                        format!("Jumlah baris chunk tidak valid: {}", e),
                                                    );
                                                        break;
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                record_failure(
                                                    &app_c,
                                                    &failures_p,
                                                    Some(table_name),
                                                    "Membaca jumlah baris chunk",
                                                    e.to_string(),
                                                );
                                                break;
                                            }
                                        };
                                        (max_pk, key_count)
                                    }
                                    Err(e) => {
                                        record_failure(
                                            &app_c,
                                            &failures_p,
                                            Some(table_name),
                                            "Mencari batas chunk data remote",
                                            e.to_string(),
                                        );
                                        break;
                                    }
                                };

                            let chunk_max = match max_pk_in_chunk {
                                Some(m) if current_last_id.is_none_or(|last_id| m > last_id) => m,
                                _ => break, // No more rows in table!
                            };

                            let where_clause = build_chunk_where_clause(
                                &safe_pk,
                                current_last_id,
                                chunk_max,
                                update_filter,
                            );

                            match dump_table_chunk_to_bytes(
                                tunnel_port,
                                &remote_cfg_c,
                                table_name,
                                &where_clause,
                                !is_fresh,
                            )
                            .await
                            {
                                Ok((bytes, rows_in_chunk)) => {
                                    total_table_rows += rows_in_chunk;
                                    if tx_c
                                        .send(PipelineMsg::DataChunk {
                                            table_name: table_name.clone(),
                                            table_index: idx + 1,
                                            chunk_num,
                                            sql_bytes: bytes,
                                            row_count: rows_in_chunk,
                                        })
                                        .await
                                        .is_err()
                                    {
                                        record_failure(
                                            &app_c,
                                            &failures_p,
                                            Some(table_name),
                                            "Mengirim data ke worker MySQL lokal",
                                            "Worker impor lokal sudah berhenti",
                                        );
                                        break;
                                    }

                                    current_last_id = Some(chunk_max);
                                    chunk_num += 1;

                                    if keys_in_chunk < this_chunk_limit as u64 {
                                        break;
                                    }
                                }
                                Err(e) => {
                                    if !is_cancelled() {
                                        record_failure(
                                            &app_c,
                                            &failures_p,
                                            Some(table_name),
                                            &format!("Dump chunk {}", chunk_num),
                                            e,
                                        );
                                    }
                                    break;
                                }
                            }
                        }
                    } else {
                        emit_log(
                            &app_c,
                            "warning",
                            format!(
                                "[Tabel '{}'] Mode incremental perlu satu primary key kolom id bertipe integer dan kolom updated_at; memakai full upsert.",
                                table_name
                            ),
                        );
                        let where_clause = match row_limit_opt {
                            Some(lim) if lim > 0 => format!("--where=1 LIMIT {}", lim),
                            _ => String::new(),
                        };
                        match dump_table_chunk_to_bytes(
                            tunnel_port,
                            &remote_cfg_c,
                            table_name,
                            &where_clause,
                            !is_fresh,
                        )
                        .await
                        {
                            Ok((bytes, rows_in_chunk)) => {
                                total_table_rows += rows_in_chunk;
                                if tx_c
                                    .send(PipelineMsg::DataChunk {
                                        table_name: table_name.clone(),
                                        table_index: idx + 1,
                                        chunk_num: 1,
                                        sql_bytes: bytes,
                                        row_count: rows_in_chunk,
                                    })
                                    .await
                                    .is_err()
                                {
                                    record_failure(
                                        &app_c,
                                        &failures_p,
                                        Some(table_name),
                                        "Mengirim data ke worker MySQL lokal",
                                        "Worker impor lokal sudah berhenti",
                                    );
                                }
                            }
                            Err(e) => {
                                if !is_cancelled() {
                                    record_failure(
                                        &app_c,
                                        &failures_p,
                                        Some(table_name),
                                        "Dump data tabel",
                                        e,
                                    );
                                }
                            }
                        }
                    }

                    if tx_c
                        .send(PipelineMsg::TableFinished {
                            table_name: table_name.clone(),
                            table_index: idx + 1,
                            total_rows_synced: total_table_rows,
                        })
                        .await
                        .is_err()
                        && !is_cancelled()
                    {
                        record_failure(
                            &app_c,
                            &failures_p,
                            Some(table_name),
                            "Menyelesaikan tabel",
                            "Worker impor lokal sudah berhenti",
                        );
                    }
                } else {
                    if tx_c
                        .send(PipelineMsg::TableFinished {
                            table_name: table_name.clone(),
                            table_index: idx + 1,
                            total_rows_synced: 0,
                        })
                        .await
                        .is_err()
                        && !is_cancelled()
                    {
                        record_failure(
                            &app_c,
                            &failures_p,
                            Some(table_name),
                            "Menyelesaikan tabel",
                            "Worker impor lokal sudah berhenti",
                        );
                    }
                }
            }
        });
        download_handles.push(handle);
    }

    // Drop the initial sender so that rx receives None when all download workers finish
    drop(tx);

    // Wait for all download workers to finish
    for h in download_handles {
        if let Err(e) = h.await {
            record_failure(
                &app,
                &failures,
                None,
                "Worker download remote",
                e.to_string(),
            );
        }
    }

    // Wait for local consumer worker to finish writing all queued data
    let (completed_table_count, total_rows_all_tables) = match consumer_handle.await {
        Ok(Ok(res)) => res,
        Ok(Err(e)) => {
            fatal_failure.store(true, Ordering::SeqCst);
            record_failure(&app, &failures, None, "Worker impor lokal", e);
            (0, 0)
        }
        Err(e) => {
            fatal_failure.store(true, Ordering::SeqCst);
            record_failure(&app, &failures, None, "Worker impor lokal", e.to_string());
            (0, 0)
        }
    };

    let _ = remote_pool.close().await;
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

    let result = SyncResult {
        success: status == "success",
        status: status.to_string(),
        total_tables: completed_table_count,
        total_rows_synced: total_rows_all_tables,
        duration_ms: duration.as_millis() as u64,
        sync_timestamp: Some(sync_timestamp),
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
    };

    let _ = app.emit("sync-finished", &result);

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
    use super::{build_chunk_where_clause, normalize_mysql_timestamp, resolve_sync_status};

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
}
