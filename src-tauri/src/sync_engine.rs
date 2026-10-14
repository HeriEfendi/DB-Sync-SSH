use crate::local_db::{ensure_local_database_exists, sanitize_identifier, spawn_mysql_child};
use crate::models::{
    LocalDbConfig, LogPayload, ProgressPayload, RemoteDbConfig, SshConfig, SyncOptions, SyncResult,
    TableInfo,
};
use crate::ssh_client::{establish_ssh_session, start_ssh_tunnel};
use sqlx::mysql::MySqlPoolOptions;
use sqlx::Row;
use std::collections::{HashMap, VecDeque};
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

pub fn emit_log<R: tauri::Runtime>(app: &tauri::AppHandle<R>, log_type: &str, message: impl Into<String>) {
    let now = chrono::Local::now().format("%H:%M:%S").to_string();
    let payload = LogPayload {
        r#type: log_type.to_string(),
        message: message.into(),
        timestamp: now,
    };
    let _ = app.emit("sync-log", payload);
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
        .map_err(|e| format!("Koneksi SSH berhasil, namun gagal terhubung ke MySQL remote: {}", e))?;

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

    if let Some(pid) = dump_child.id() {
        register_child_pid(pid);
    }

    let mut output_bytes = Vec::new();
    let mut rows_count = 0usize;

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
                    return Err(format!("Gagal membaca stream mysqldump: {}", e));
                }
            }
        }
    }

    let _ = dump_child.wait().await;
    if let Some(pid) = dump_child.id() {
        unregister_child_pid(pid);
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

    emit_log(&app, "info", "🚀 Memulai inisialisasi sinkronisasi DB over SSH...");

    // 1. Ensure local database exists
    if let Err(e) = ensure_local_database_exists(&local_db_config).await {
        emit_log(&app, "error", format!("Gagal memverifikasi database lokal: {}", e));
        return Err(e);
    }

    // 2. Open SSH port forward tunnel
    emit_log(&app, "info", format!("🔌 Menghubungkan SSH tunnel ke {}...", ssh_config.host));
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
            emit_log(&app, "error", format!("Koneksi MySQL remote via tunnel gagal: {}", e));
            return Err(e.to_string());
        }
    };

    // Pre-query primary key columns of remote tables in 1 quick batch query
    let pk_query = "
        SELECT TABLE_NAME, COLUMN_NAME 
        FROM information_schema.COLUMNS 
        WHERE TABLE_SCHEMA = DATABASE() AND COLUMN_KEY = 'PRI'
        ORDER BY ORDINAL_POSITION ASC
    ";
    let mut pk_map: HashMap<String, String> = HashMap::new();
    if let Ok(rows) = sqlx::query(pk_query).fetch_all(&*remote_pool).await {
        for r in rows {
            let t_name: String = r.try_get("TABLE_NAME").unwrap_or_default();
            let c_name: String = r.try_get("COLUMN_NAME").unwrap_or_default();
            if !t_name.is_empty() && !c_name.is_empty() && !pk_map.contains_key(&t_name) {
                pk_map.insert(t_name, c_name);
            }
        }
    }
    let pk_map = Arc::new(pk_map);

    let total_tables = sync_options.tables.len();
    let sync_mode = sync_options.sync_mode.as_deref().unwrap_or("incremental");
    let is_fresh = sync_mode == "fresh";
    let download_concurrency = if total_tables >= 2 { 2 } else { 1 };

    emit_log(
        &app,
        "success",
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
        sync_options.tables.into_iter().enumerate().collect::<VecDeque<_>>()
    ));
    let incremental_watermarks = Arc::new(sync_options.incremental_watermarks);
    let remote_db_cfg = Arc::new(remote_db_config);

    // ----------------------------------------------------
    // Consumer Task: Local MySQL Importer Worker
    // ----------------------------------------------------
    let local_cfg_c = local_db_config.clone();
    let app_consumer = app.clone();
    let total_tables_c = total_tables;

    let consumer_handle = tokio::spawn(async move {
        let (mut local_child, mut local_stdin, _) = match spawn_mysql_child(&local_cfg_c) {
            Ok(c) => c,
            Err(e) => {
                emit_log(&app_consumer, "error", format!("Gagal menjalankan proses import MySQL lokal: {}", e));
                return Err(e);
            }
        };
        if let Some(pid) = local_child.id() {
            register_child_pid(pid);
        }

        use tokio::io::AsyncWriteExt;
        let prelude = "SET SESSION sql_log_bin=0;\nSET FOREIGN_KEY_CHECKS=0;\nSET UNIQUE_CHECKS=0;\nSET AUTOCOMMIT=1;\n";
        let _ = local_stdin.write_all(prelude.as_bytes()).await;

        let mut total_rows_imported = 0usize;
        let mut completed_tables = 0usize;

        while let Some(msg) = rx.recv().await {
            if is_cancelled() {
                break;
            }
            match msg {
                PipelineMsg::Ddl { table_name, table_index: _, sql } => {
                    if let Err(e) = local_stdin.write_all(sql.as_bytes()).await {
                        emit_log(&app_consumer, "error", format!("[Tabel '{}'] Gagal menerapkan DDL lokal: {}", table_name, e));
                    } else {
                        let _ = local_stdin.flush().await;
                        emit_log(&app_consumer, "success", format!("[Tabel '{}'] ✅ Struktur DDL diterapkan ke MySQL lokal.", table_name));
                    }
                }
                PipelineMsg::DataChunk { table_name, table_index, chunk_num, sql_bytes, row_count } => {
                    if !sql_bytes.is_empty() {
                        if let Err(e) = local_stdin.write_all(&sql_bytes).await {
                            emit_log(&app_consumer, "error", format!("[Tabel '{}'] Gagal menulis data chunk ke MySQL lokal: {}", table_name, e));
                        } else {
                            let _ = local_stdin.flush().await;
                        }
                    }
                    total_rows_imported += row_count;
                    emit_progress(
                        &app_consumer,
                        table_index,
                        total_tables_c,
                        &table_name,
                        row_count,
                        total_rows_imported,
                        "syncing",
                    );
                    if row_count > 0 {
                        emit_log(
                            &app_consumer,
                            "info",
                            format!(
                                "[Tabel '{}'] 📦 Chunk {} dialirkan (~{} baris)...",
                                table_name,
                                chunk_num,
                                format_number(row_count)
                            ),
                        );
                    }
                }
                PipelineMsg::TableFinished { table_name, table_index, total_rows_synced } => {
                    completed_tables += 1;
                    emit_log(
                        &app_consumer,
                        "success",
                        format!(
                            "[Tabel '{}'] ✅ Selesai: total ~{} row berhasil disalin.",
                            table_name,
                            format_number(total_rows_synced)
                        ),
                    );
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
        let _ = local_stdin.write_all(postlude.as_bytes()).await;
        let _ = local_stdin.flush().await;
        drop(local_stdin);

        let _ = local_child.wait().await;
        if let Some(pid) = local_child.id() {
            unregister_child_pid(pid);
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
        let watermarks_c = incremental_watermarks.clone();
        let remote_cfg_c = remote_db_cfg.clone();
        let app_c = app.clone();
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
                        emit_log(&app_c, "error", format!("Nama tabel tidak valid '{}': {}", table_name, e));
                        continue;
                    }
                };

                // A. Structure DDL
                if table_item.sync_structure {
                    emit_log(
                        &app_c,
                        "info",
                        format!("[{}/{}] 🏗️ [Worker {}] Mengambil DDL tabel '{}'...", idx + 1, total_tables, worker_id + 1, table_name),
                    );

                    let q = format!("SHOW CREATE TABLE {}", safe_table);
                    if let Ok(row) = sqlx::query(&q).fetch_one(&*pool_c).await {
                        let ddl: String = row.try_get(1).unwrap_or_default();
                        if !ddl.is_empty() {
                            let ddl_sql = if is_fresh {
                                format!(
                                    "SET FOREIGN_KEY_CHECKS=0;\nDROP TABLE IF EXISTS {};\n{};\nSET FOREIGN_KEY_CHECKS=1;\n",
                                    safe_table, ddl
                                )
                            } else {
                                let safe_create = if !ddl.to_uppercase().contains("CREATE TABLE IF NOT EXISTS") {
                                    ddl.replacen("CREATE TABLE", "CREATE TABLE IF NOT EXISTS", 1)
                                } else {
                                    ddl
                                };
                                format!(
                                    "SET FOREIGN_KEY_CHECKS=0;\n{};\nSET FOREIGN_KEY_CHECKS=1;\n",
                                    safe_create
                                )
                            };
                            let _ = tx_c.send(PipelineMsg::Ddl {
                                table_name: table_name.clone(),
                                table_index: idx + 1,
                                sql: ddl_sql,
                            }).await;
                        }
                    }
                }

                // B. Data Sync (with Chunking for large tables)
                if table_item.sync_data {
                    let pk_col_opt = pk_map_c.get(table_name).cloned();
                    let watermark = if !is_fresh {
                        watermarks_c.as_ref().as_ref().and_then(|w| w.get(table_name))
                    } else {
                        None
                    };

                    let start_id = match watermark {
                        Some(wm) if wm.last_id > 0 && !wm.primary_key.trim().is_empty() => wm.last_id,
                        _ => 0,
                    };

                    let mut total_table_rows = 0usize;

                    if let Some(ref pk_col) = pk_col_opt {
                        let safe_pk = sanitize_identifier(pk_col).unwrap_or_else(|_| format!("`{}`", pk_col));
                        let mut current_last_id = start_id;
                        let mut chunk_num = 1;

                        if start_id > 0 {
                            emit_log(
                                &app_c,
                                "info",
                                format!(
                                    "[{}/{}] ⚡ [Worker {}] Sync New & Update '{}' setelah ID {} (PK: {})...",
                                    idx + 1,
                                    total_tables,
                                    worker_id + 1,
                                    table_name,
                                    format_number(start_id as usize),
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
                            let bound_q = format!(
                                "SELECT MAX(t.pk) FROM (SELECT {} AS pk FROM {} WHERE {} > {} ORDER BY {} ASC LIMIT {}) AS t",
                                safe_pk, safe_table, safe_pk, current_last_id, safe_pk, this_chunk_limit
                            );

                            let max_pk_in_chunk: Option<i64> = match sqlx::query(&bound_q).fetch_one(&*pool_c).await {
                                Ok(row) => {
                                    if let Ok(val) = row.try_get::<i64, _>(0) {
                                        Some(val)
                                    } else if let Ok(val) = row.try_get::<u64, _>(0) {
                                        Some(val as i64)
                                    } else if let Ok(val) = row.try_get::<i32, _>(0) {
                                        Some(val as i64)
                                    } else {
                                        None
                                    }
                                }
                                Err(_) => None,
                            };

                            let chunk_max = match max_pk_in_chunk {
                                Some(m) if m > current_last_id => m,
                                _ => break, // No more rows in table!
                            };

                            let where_clause = if current_last_id == 0 {
                                format!("--where={} <= {}", safe_pk, chunk_max)
                            } else {
                                format!("--where={} > {} AND {} <= {}", safe_pk, current_last_id, safe_pk, chunk_max)
                            };

                            match dump_table_chunk_to_bytes(
                                tunnel_port,
                                &remote_cfg_c,
                                table_name,
                                &where_clause,
                                !is_fresh,
                            ).await {
                                Ok((bytes, rows_in_chunk)) => {
                                    let effective_rows = if rows_in_chunk == 0 { this_chunk_limit } else { rows_in_chunk };
                                    total_table_rows += effective_rows;

                                    let _ = tx_c.send(PipelineMsg::DataChunk {
                                        table_name: table_name.clone(),
                                        table_index: idx + 1,
                                        chunk_num,
                                        sql_bytes: bytes,
                                        row_count: effective_rows,
                                    }).await;

                                    current_last_id = chunk_max;
                                    chunk_num += 1;

                                    if rows_in_chunk < this_chunk_limit {
                                        break;
                                    }
                                }
                                Err(e) => {
                                    emit_log(&app_c, "error", format!("[Tabel '{}'] Gagal dump chunk {}: {}", table_name, chunk_num, e));
                                    break;
                                }
                            }
                        }
                    } else {
                        // Table has no single PK: dump as single chunk
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
                        ).await {
                            Ok((bytes, rows_in_chunk)) => {
                                total_table_rows += rows_in_chunk;
                                let _ = tx_c.send(PipelineMsg::DataChunk {
                                    table_name: table_name.clone(),
                                    table_index: idx + 1,
                                    chunk_num: 1,
                                    sql_bytes: bytes,
                                    row_count: rows_in_chunk,
                                }).await;
                            }
                            Err(e) => {
                                emit_log(&app_c, "error", format!("[Tabel '{}'] Gagal dump: {}", table_name, e));
                            }
                        }
                    }

                    let _ = tx_c.send(PipelineMsg::TableFinished {
                        table_name: table_name.clone(),
                        table_index: idx + 1,
                        total_rows_synced: total_table_rows,
                    }).await;
                } else {
                    let _ = tx_c.send(PipelineMsg::TableFinished {
                        table_name: table_name.clone(),
                        table_index: idx + 1,
                        total_rows_synced: 0,
                    }).await;
                }
            }
        });
        download_handles.push(handle);
    }

    // Drop the initial sender so that rx receives None when all download workers finish
    drop(tx);

    // Wait for all download workers to finish
    for h in download_handles {
        let _ = h.await;
    }

    // Wait for local consumer worker to finish writing all queued data
    let (completed_table_count, total_rows_all_tables) = match consumer_handle.await {
        Ok(Ok(res)) => res,
        _ => (0, 0),
    };

    let _ = remote_pool.close().await;
    drop(tunnel);

    let duration = start_time.elapsed();
    let was_cancelled = is_cancelled();

    let result = SyncResult {
        success: !was_cancelled,
        total_tables: completed_table_count,
        total_rows_synced: total_rows_all_tables,
        duration_ms: duration.as_millis() as u64,
        error: if was_cancelled {
            Some("Dibatalkan oleh pengguna".to_string())
        } else {
            None
        },
    };

    let _ = app.emit("sync-finished", &result);

    if was_cancelled {
        emit_log(
            &app,
            "warning",
            format!(
                "⚠️ Sinkronisasi dibatalkan. {} tabel diproses, {} row disalin ({:.1}s).",
                completed_table_count,
                format_number(total_rows_all_tables),
                duration.as_secs_f64()
            ),
        );
    } else {
        emit_log(
            &app,
            "success",
            format!(
                "🎉 Pipeline Sync Berhasil! {} tabel diproses, total {} row disalin dalam {:.1} detik.",
                completed_table_count,
                format_number(total_rows_all_tables),
                duration.as_secs_f64()
            ),
        );
    }

    Ok(result)
}
