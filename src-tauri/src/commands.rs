use crate::local_db::{get_local_tables as get_local_tables_db, test_local_connection as test_local_db};
use crate::models::{
    LocalDbConfig, RemoteDbConfig, SshConfig, SyncOptions, SyncResult, TableInfo,
};
use crate::sync_engine::{
    cancel_sync as cancel_sync_engine, get_remote_tables as get_remote_tables_engine, run_sync_task,
    test_ssh_and_remote_db as test_ssh_engine,
};

#[tauri::command]
pub async fn test_ssh_and_remote_db(
    ssh_config: SshConfig,
    remote_db_config: RemoteDbConfig,
) -> Result<String, String> {
    test_ssh_engine(&ssh_config, &remote_db_config).await
}

#[tauri::command]
pub async fn test_local_connection(config: LocalDbConfig) -> Result<String, String> {
    test_local_db(&config).await
}

#[tauri::command]
pub async fn get_remote_tables(
    ssh_config: SshConfig,
    remote_db_config: RemoteDbConfig,
) -> Result<Vec<TableInfo>, String> {
    get_remote_tables_engine(&ssh_config, &remote_db_config).await
}

#[tauri::command]
pub async fn get_local_tables(config: LocalDbConfig) -> Result<Vec<String>, String> {
    get_local_tables_db(&config).await
}

#[tauri::command]
pub async fn drop_local_tables(config: LocalDbConfig, tables: Vec<String>) -> Result<Vec<String>, String> {
    crate::local_db::drop_local_tables(&config, tables).await
}

#[tauri::command]
pub async fn compare_table_counts(
    ssh_config: SshConfig,
    remote_db_config: RemoteDbConfig,
    local_db_config: LocalDbConfig,
    tables: Option<Vec<String>>,
) -> Result<Vec<crate::models::TableDiffInfo>, String> {
    crate::sync_engine::compare_table_counts(
        &ssh_config,
        &remote_db_config,
        &local_db_config,
        tables,
    )
    .await
}

#[tauri::command]
pub async fn get_all_tables_last_local_ids(
    config: LocalDbConfig,
    tables: Option<Vec<String>>,
) -> Result<Vec<crate::models::TableLastIdInfo>, String> {
    crate::local_db::get_all_tables_last_local_ids(&config, tables).await
}

#[tauri::command]
pub async fn start_ssh_sync(
    app: tauri::AppHandle,
    ssh_config: SshConfig,
    remote_db_config: RemoteDbConfig,
    local_db_config: LocalDbConfig,
    sync_options: SyncOptions,
) -> Result<SyncResult, String> {
    run_sync_task(app, ssh_config, remote_db_config, local_db_config, sync_options).await
}

#[tauri::command]
pub fn cancel_ssh_sync() -> Result<(), String> {
    cancel_sync_engine();
    Ok(())
}

#[tauri::command]
pub async fn pick_private_key_file() -> Result<Option<String>, String> {
    let file = rfd::AsyncFileDialog::new()
        .set_title("Pilih Private Key SSH")
        .pick_file()
        .await;

    Ok(file.map(|f| f.path().to_string_lossy().to_string()))
}
