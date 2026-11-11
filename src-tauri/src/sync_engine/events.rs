use crate::models::{LogPayload, ProgressPayload, SyncFailure};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tauri::Emitter;

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

pub fn record_failure<R: tauri::Runtime>(
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

pub fn table_has_failure(failures: &Arc<Mutex<Vec<SyncFailure>>>, table_name: &str) -> bool {
    failures
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .iter()
        .any(|failure| failure.table_name.as_deref() == Some(table_name))
}

pub fn table_has_no_data_to_sync(sync_data: bool, rows_synced: usize, has_failure: bool) -> bool {
    sync_data && rows_synced == 0 && !has_failure
}

pub fn progress_table_counts(
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

pub fn resolve_sync_status(
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

