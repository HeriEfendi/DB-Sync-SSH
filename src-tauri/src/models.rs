use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SshConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    /// "key_no_pass" | "key_with_passphrase" | "password" | "agent"
    pub auth_method: String,
    pub private_key_path: Option<String>,
    pub passphrase: Option<String>,
    pub password: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RemoteDbConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub database: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LocalDbConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub database: String,
    #[serde(default)]
    pub use_docker: bool,
    #[serde(default)]
    pub docker_container: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TableInfo {
    pub name: String,
    pub total_rows: u64,
    pub data_size_bytes: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TableSyncItem {
    pub name: String,
    pub sync_structure: bool,
    pub sync_data: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WatermarkItem {
    pub primary_key: String,
    pub last_id: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TableLastIdInfo {
    pub table_name: String,
    pub primary_key: Option<String>,
    pub max_id: Option<i64>,
    pub max_id_str: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SyncOptions {
    pub tables: Vec<TableSyncItem>,
    pub row_limit: Option<usize>,
    pub throttle_ms: Option<u64>,
    #[serde(default)]
    pub sync_mode: Option<String>, // "incremental" | "fresh"
    #[serde(default)]
    pub incremental_watermarks: Option<std::collections::HashMap<String, WatermarkItem>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LogPayload {
    pub r#type: String, // "info" | "success" | "warning" | "error"
    pub message: String,
    pub timestamp: String,
    pub details: Option<String>,
    pub table_name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProgressPayload {
    pub current_table_index: usize,
    pub total_tables: usize,
    pub current_table_name: String,
    pub rows_synced_current_table: usize,
    pub total_synced_all_tables: usize,
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SyncResult {
    pub success: bool,
    pub status: String,
    pub total_tables: usize,
    pub total_rows_synced: usize,
    pub duration_ms: u64,
    pub error: Option<String>,
    pub failures: Vec<SyncFailure>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SyncFailure {
    pub table_name: Option<String>,
    pub stage: String,
    pub message: String,
}
