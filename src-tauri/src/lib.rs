pub mod commands;
pub mod local_db;
pub mod models;
pub mod process_utils;
pub mod ssh_client;
pub mod sync_engine;

pub use commands::*;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::test_ssh_and_remote_db,
            commands::test_local_connection,
            commands::get_remote_tables,
            commands::get_local_tables,
            commands::drop_local_tables,
            commands::compare_table_counts,
            commands::get_all_tables_last_local_ids,
            commands::start_ssh_sync,
            commands::cancel_ssh_sync,
            commands::pick_private_key_file
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
