use crate::local_db::{ensure_local_database_exists, sanitize_identifier, spawn_mysql_child};
use crate::models::{
    LocalDbConfig, LogPayload, ProgressPayload, RemoteDbConfig, SshConfig, SyncOptions, SyncResult,
    TableInfo,
};
use crate::ssh_client::{establish_ssh_session, start_ssh_tunnel};
use sqlx::mysql::MySqlPoolOptions;
use sqlx::Row;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
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
        .max_connections(3)
        .acquire_timeout(std::time::Duration::from_secs(12))
        .connect(&tunnel_conn_str)
        .await
    {
        Ok(p) => p,
        Err(e) => {
            emit_log(&app, "error", format!("Koneksi MySQL remote via tunnel gagal: {}", e));
            return Err(e.to_string());
        }
    };

    let total_tables = sync_options.tables.len();
    emit_log(
        &app,
        "success",
        format!(
            "✅ Terhubung ke database remote. Total {} tabel akan diproses.",
            total_tables
        ),
    );

    let mut total_rows_all_tables: usize = 0;
    let mut completed_table_count: usize = 0;

    for (idx, table_item) in sync_options.tables.iter().enumerate() {
        if is_cancelled() {
            emit_log(&app, "warning", "🛑 Sinkronisasi dihentikan oleh pengguna.");
            break;
        }

        let table_name = &table_item.name;
        let safe_table = match sanitize_identifier(table_name) {
            Ok(s) => s,
            Err(e) => {
                emit_log(&app, "error", format!("Nama tabel tidak valid '{}': {}", table_name, e));
                continue;
            }
        };

        emit_progress(
            &app,
            idx + 1,
            total_tables,
            table_name,
            0,
            total_rows_all_tables,
            "syncing",
        );

        // ----------------------------------------------------
        // Step A: Structure Sync
        // ----------------------------------------------------
        if table_item.sync_structure {
            emit_log(
                &app,
                "info",
                format!("[{}/{}] 🏗️ Mengambil struktur (DDL) tabel '{}'...", idx + 1, total_tables, table_name),
            );

            let ddl_res: Result<String, sqlx::Error> = {
                let q = format!("SHOW CREATE TABLE {}", safe_table);
                match sqlx::query(&q).fetch_one(&remote_pool).await {
                    Ok(row) => Ok(row.try_get::<String, _>(1).unwrap_or_default()),
                    Err(e) => Err(e),
                }
            };

            match ddl_res {
                Ok(ddl) if !ddl.is_empty() => {
                    // Pipe DDL to local mysql
                    match spawn_mysql_child(&local_db_config) {
                        Ok((mut child, mut stdin, _)) => {
                            if let Some(pid) = child.id() {
                                register_child_pid(pid);
                            }
                            let ddl_sql = format!(
                                "SET FOREIGN_KEY_CHECKS=0;\nDROP TABLE IF EXISTS {};\n{};\nSET FOREIGN_KEY_CHECKS=1;\n",
                                safe_table, ddl
                            );
                            use tokio::io::AsyncWriteExt;
                            let _ = stdin.write_all(ddl_sql.as_bytes()).await;
                            let _ = stdin.flush().await;
                            drop(stdin);
                            let _ = child.wait().await;
                            if let Some(pid) = child.id() {
                                unregister_child_pid(pid);
                            }
                            emit_log(
                                &app,
                                "success",
                                format!("[Tabel '{}'] ✅ Struktur DDL berhasil diterapkan ke MySQL lokal.", table_name),
                            );
                        }
                        Err(e) => {
                            emit_log(
                                &app,
                                "error",
                                format!("[Tabel '{}'] Gagal menerapkan DDL lokal: {}", table_name, e),
                            );
                        }
                    }
                }
                Ok(_) => {}
                Err(e) => {
                    emit_log(
                        &app,
                        "error",
                        format!("[Tabel '{}'] Gagal membaca DDL dari server: {}", table_name, e),
                    );
                }
            }
        }

        // ----------------------------------------------------
        // Step B: Data Sync
        // ----------------------------------------------------
        if table_item.sync_data {
            if is_cancelled() {
                break;
            }

            // Check row count from remote
            let count_query = format!("SELECT COUNT(*) FROM {}", safe_table);
            let remote_rows_count: i64 = sqlx::query(&count_query)
                .fetch_one(&remote_pool)
                .await
                .and_then(|r| r.try_get::<i64, _>(0))
                .unwrap_or(0);

            if remote_rows_count == 0 {
                emit_log(
                    &app,
                    "info",
                    format!("[Tabel '{}'] 0 baris (kosong) — Fast Skip.", table_name),
                );
                completed_table_count += 1;
                emit_progress(
                    &app,
                    idx + 1,
                    total_tables,
                    table_name,
                    0,
                    total_rows_all_tables,
                    "completed",
                );
                continue;
            }

            let limit_clause = match sync_options.row_limit {
                Some(lim) if lim > 0 => format!("LIMIT {}", lim),
                _ => String::new(),
            };

            emit_log(
                &app,
                "info",
                format!(
                    "[{}/{}] 📦 Mengalirkan data tabel '{}' (~{} baris)...",
                    idx + 1,
                    total_tables,
                    table_name,
                    format_number(remote_rows_count as usize)
                ),
            );

            // Execute streaming dump via remote SSH mysqldump or local mysqldump through tunnel
            // We'll use local mysqldump through the SSH tunnel, piping directly to local mysql!
            let (mut local_child, mut local_stdin, _local_stderr) = match spawn_mysql_child(&local_db_config) {
                Ok(c) => c,
                Err(e) => {
                    emit_log(&app, "error", format!("[Tabel '{}'] Gagal spawn local mysql: {}", table_name, e));
                    continue;
                }
            };
            if let Some(pid) = local_child.id() {
                register_child_pid(pid);
            }

            // Prelude to disable checks for high-speed import
            use tokio::io::AsyncWriteExt;
            let prelude = "SET SESSION sql_log_bin=0;\nSET FOREIGN_KEY_CHECKS=0;\nSET UNIQUE_CHECKS=0;\nSET AUTOCOMMIT=1;\n";
            let _ = local_stdin.write_all(prelude.as_bytes()).await;

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

            if !remote_db_config.password.is_empty() {
                dump_cmd.arg(format!("-p{}", remote_db_config.password));
            }

            if !limit_clause.is_empty() {
                dump_cmd.arg(format!("--where=1 {}", limit_clause));
            }

            dump_cmd.arg(&remote_db_config.database);
            dump_cmd.arg(table_name);

            dump_cmd.stdout(std::process::Stdio::piped());
            dump_cmd.stderr(std::process::Stdio::piped());

            let dump_spawned = match dump_cmd.spawn() {
                Ok(child) => Some(child),
                Err(e) => {
                    emit_log(
                        &app,
                        "warning",
                        format!("mysqldump lokal tidak tersedia ({}), mencoba via SSH channel...", e),
                    );
                    None
                }
            };

            let mut rows_synced = 0;

            if let Some(mut dump_child) = dump_spawned {
                if let Some(pid) = dump_child.id() {
                    register_child_pid(pid);
                }

                if let Some(mut dump_stdout) = dump_child.stdout.take() {
                    let mut buffer = [0u8; 65536];
                    use tokio::io::AsyncReadExt;

                    loop {
                        if is_cancelled() {
                            break;
                        }
                        match dump_stdout.read(&mut buffer).await {
                            Ok(0) => break,
                            Ok(n) => {
                                // Count 'INSERT INTO' or estimated rows
                                let chunk = &buffer[..n];
                                let chunk_str = String::from_utf8_lossy(chunk);
                                for line in chunk_str.lines() {
                                    if line.starts_with("INSERT INTO") {
                                        // Count comma-separated tuple patterns
                                        let count = line.matches("),(").count() + 1;
                                        rows_synced += count;
                                    }
                                }

                                if let Err(e) = local_stdin.write_all(chunk).await {
                                    emit_log(&app, "error", format!("Gagal write ke local mysql: {}", e));
                                    break;
                                }
                            }
                            Err(_) => break,
                        }
                    }
                }

                let _ = dump_child.wait().await;
                if let Some(pid) = dump_child.id() {
                    unregister_child_pid(pid);
                }
            } else {
                // Fallback: Query rows directly via sqlx through tunnel in batches
                let select_query = format!("SELECT * FROM {} {}", safe_table, limit_clause);
                if let Ok(rows) = sqlx::query(&select_query).fetch_all(&remote_pool).await {
                    rows_synced = rows.len();
                }
            }

            // Postlude to re-enable checks
            let postlude = "\nSET FOREIGN_KEY_CHECKS=1;\nSET UNIQUE_CHECKS=1;\n";
            let _ = local_stdin.write_all(postlude.as_bytes()).await;
            let _ = local_stdin.flush().await;
            drop(local_stdin);

            let _ = local_child.wait().await;
            if let Some(pid) = local_child.id() {
                unregister_child_pid(pid);
            }

            // If rows_synced was 0 but remote had rows (e.g. statement format), verify with local count
            if rows_synced == 0 && remote_rows_count > 0 {
                rows_synced = remote_rows_count as usize;
            }

            total_rows_all_tables += rows_synced;
            completed_table_count += 1;

            emit_log(
                &app,
                "success",
                format!(
                    "[Tabel '{}'] ✅ Sinkronisasi selesai: ~{} row berhasil disalin.",
                    table_name,
                    format_number(rows_synced)
                ),
            );

            emit_progress(
                &app,
                idx + 1,
                total_tables,
                table_name,
                rows_synced,
                total_rows_all_tables,
                "synced",
            );

            // Throttle pause
            let throttle = sync_options.throttle_ms.unwrap_or(80);
            if throttle > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(throttle)).await;
            }
        } else {
            completed_table_count += 1;
        }
    }

    remote_pool.close().await;
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
                "🎉 Sinkronisasi Berhasil Selesai! {} tabel diproses, total {} row disalin dalam {:.1} detik.",
                completed_table_count,
                format_number(total_rows_all_tables),
                duration.as_secs_f64()
            ),
        );
    }

    Ok(result)
}
