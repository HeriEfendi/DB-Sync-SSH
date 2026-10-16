use serde::{Deserialize, Serialize};

pub fn supports_incremental_integer_primary_key(data_type: &str) -> bool {
    matches!(
        data_type.to_ascii_lowercase().as_str(),
        "tinyint" | "smallint" | "mediumint" | "int" | "integer" | "bigint"
    )
}

pub fn incremental_primary_key(columns: &[(String, String, String)]) -> Option<String> {
    if columns.len() != 1 {
        return None;
    }
    let (column, data_type, _) = &columns[0];
    (column.eq_ignore_ascii_case("id") && supports_incremental_integer_primary_key(data_type))
        .then(|| column.clone())
}

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
    #[serde(deserialize_with = "deserialize_integer_string")]
    pub last_id: String,
    #[serde(default)]
    pub last_sync_time: Option<String>,
}

fn deserialize_integer_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum IntegerValue {
        Signed(i64),
        Unsigned(u64),
        Text(String),
    }

    match IntegerValue::deserialize(deserializer)? {
        IntegerValue::Signed(value) => Ok(value.to_string()),
        IntegerValue::Unsigned(value) => Ok(value.to_string()),
        IntegerValue::Text(value) => Ok(value),
    }
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
    pub completed_tables: usize,
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
    pub sync_timestamp: Option<String>,
    pub error: Option<String>,
    pub failures: Vec<SyncFailure>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SyncFailure {
    pub table_name: Option<String>,
    pub stage: String,
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::incremental_primary_key;

    #[test]
    fn incremental_keyset_accepts_one_integer_id_primary_key() {
        let columns = vec![(
            "id".to_string(),
            "bigint".to_string(),
            "bigint unsigned".to_string(),
        )];
        assert_eq!(incremental_primary_key(&columns).as_deref(), Some("id"));
    }

    #[test]
    fn incremental_keyset_rejects_text_non_id_and_composite_primary_keys() {
        let text_key = vec![(
            "id".to_string(),
            "varchar".to_string(),
            "varchar(36)".to_string(),
        )];
        let composite_key = vec![
            (
                "tenant_id".to_string(),
                "int".to_string(),
                "int".to_string(),
            ),
            (
                "record_id".to_string(),
                "int".to_string(),
                "int".to_string(),
            ),
        ];
        let non_id_key = vec![(
            "record_id".to_string(),
            "int".to_string(),
            "int".to_string(),
        )];
        assert_eq!(incremental_primary_key(&text_key), None);
        assert_eq!(incremental_primary_key(&composite_key), None);
        assert_eq!(incremental_primary_key(&non_id_key), None);
    }
}
