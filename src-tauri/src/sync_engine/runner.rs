use crate::local_db::{
    build_local_connection_string, ensure_local_database_exists,
    format_local_mysql_connection_error, sanitize_identifier, spawn_mysql_child,
};
use crate::models::{
    incremental_primary_key, LocalDbConfig, LogPayload, RemoteDbConfig, SshConfig, SyncFailure,
    SyncOptions, SyncResult,
};
use crate::ssh_client::start_ssh_tunnel;
use crate::sync_engine::cancellation::{
    is_cancelled, register_child_pid, reset_cancel_state, unregister_child_pid,
};
use crate::sync_engine::events::{
    emit_log, emit_progress, progress_table_counts, record_failure, resolve_sync_status,
    table_has_failure, table_has_no_data_to_sync,
};
use crate::sync_engine::importer::{check_importer_running, stream_dump_to_local_importer};
use crate::sync_engine::sql_helpers::{
    extract_clean_mysql_error, format_number, has_mysql_cli_errors, is_schema_mismatch_error,
    normalize_mysql_timestamp,
};
use sqlx::mysql::MySqlPoolOptions;
use sqlx::Row;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tauri::Emitter;

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

    emit_log(
        &app,
        "info",
        "🔎 Memeriksa MySQL/MariaDB client lokal yang diperlukan untuk import...",
    );
    if !local_db_config.use_docker {
        if let Err(error) =
            crate::process_utils::resolve_mysql_binary(&["mariadb", "mysql"])
        {
            emit_log(&app, "error", &error);
            return Err(error);
        }
    }
    let remote_dump_binary = if sync_options.tables.iter().any(|table| table.sync_data) {
        match crate::process_utils::resolve_mysql_binary(&["mariadb-dump", "mysqldump"]) {
            Ok(binary) => Some(binary),
            Err(error) => {
                emit_log(&app, "error", &error);
                return Err(error);
            }
        }
    } else {
        None
    };
    emit_log(
        &app,
        "success",
        "✅ MySQL/MariaDB client yang diperlukan tersedia.",
    );

    // 1. Ensure local database exists
    emit_log(&app, "info", "🗄️ Memastikan database MySQL lokal siap...");
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

    emit_log(&app, "info", "🔗 Tunnel SSH siap; menghubungkan ke MySQL remote...");
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
    emit_log(&app, "info", "📋 Membaca metadata skema database lokal...");
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
    emit_log(&app, "success", "✅ Metadata skema lokal selesai dibaca.");

    // 2. Batch query remote schema metadata: PKs, updated_at columns, and full schema signatures
    emit_log(&app, "info", "📋 Membaca metadata skema database remote...");
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
    emit_log(&app, "success", "✅ Metadata remote selesai; menyiapkan daftar tabel...");

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
            emit_log(
                &app,
                "info",
                "🔍 Memeriksa perubahan incremental pada tabel-tabel terpilih...",
            );
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
                    if is_cancelled() {
                        break;
                    }
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

                    let query_fut = sqlx::query(&union_sql).fetch_all(&*remote_pool);
                    match tokio::time::timeout(std::time::Duration::from_secs(10), query_fut).await {
                        Ok(Ok(rows)) => {
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
                        Ok(Err(err)) => {
                            eprintln!("[BatchWatermarkCheck] Batch query failed, skipping fast-check for this batch: {}", err);
                        }
                        Err(_) => {
                            eprintln!("[BatchWatermarkCheck] Batch query timed out (>10s), continuing to worker pipeline");
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
    emit_log(&app, "success", "✅ Pemeriksaan awal tabel selesai; menyiapkan worker sinkronisasi...");
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
        let dump_binary_c = remote_dump_binary.clone();
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

                    let dump_binary = match dump_binary_c.as_deref() {
                        Some(binary) => binary,
                        None => {
                            let error = "Executable dump MySQL remote tidak disiapkan untuk sinkronisasi data.";
                            record_failure(
                                &app_c,
                                &failures_c,
                                Some(table_name),
                                "Menyiapkan importer data remote",
                                error,
                            );
                            fatal_failure_c.store(true, Ordering::SeqCst);
                            break;
                        }
                    };
                    match stream_dump_to_local_importer(
                        &app_c,
                        tunnel_port,
                        &remote_cfg_c,
                        table_name,
                        idx + 1,
                        dump_binary,
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
