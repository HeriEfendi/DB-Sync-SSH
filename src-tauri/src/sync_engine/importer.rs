use crate::models::{RemoteDbConfig, SyncFailure};
use crate::sync_engine::cancellation::{is_cancelled, register_child_pid, unregister_child_pid};
use crate::sync_engine::events::{emit_progress, record_failure};
use crate::sync_engine::row_counter::MysqlDumpRowCounter;
use crate::sync_engine::sql_helpers::get_dump_compression_args;
use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

pub fn check_importer_running<R: tauri::Runtime>(
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

pub async fn stream_dump_to_local_importer<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    tunnel_port: u16,
    remote_db_config: &RemoteDbConfig,
    table_name: &str,
    table_index: usize,
    dump_binary: &Path,
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
    let dump_bin = dump_binary.to_string_lossy();
    let mut dump_cmd = crate::process_utils::new_tokio_command(dump_binary);
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

    for arg in get_dump_compression_args(&dump_bin) {
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

