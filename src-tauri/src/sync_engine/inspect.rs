use crate::local_db::{
    build_local_connection_string, format_local_mysql_connection_error, sanitize_identifier,
};
use crate::models::{LocalDbConfig, RemoteDbConfig, SshConfig, TableDiffInfo, TableInfo};
use crate::ssh_client::{establish_ssh_session, start_ssh_tunnel};
use sqlx::mysql::MySqlPoolOptions;
use sqlx::Row;
use std::collections::HashMap;

pub async fn test_ssh_and_remote_db(
    ssh_config: &SshConfig,
    remote_db_config: &RemoteDbConfig,
) -> Result<String, String> {
    let session = establish_ssh_session(ssh_config)?;
    let tunnel = start_ssh_tunnel(ssh_config, remote_db_config)?;
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let tunnel_conn_str = format!(
        "mysql://{}:{}@127.0.0.1:{}/{}",
        urlencoding::encode(&remote_db_config.username),
        urlencoding::encode(&remote_db_config.password),
        tunnel.local_port,
        urlencoding::encode(&remote_db_config.database)
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
        urlencoding::encode(&remote_db_config.database)
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

pub async fn compare_table_counts(
    ssh_config: &SshConfig,
    remote_db_config: &RemoteDbConfig,
    local_db_config: &LocalDbConfig,
    tables: Option<Vec<String>>,
) -> Result<Vec<TableDiffInfo>, String> {
    let local_counts: HashMap<String, i64> = {
        let conn_str = build_local_connection_string(local_db_config);
        let pool = MySqlPoolOptions::new()
            .max_connections(2)
            .acquire_timeout(std::time::Duration::from_secs(6))
            .connect(&conn_str)
            .await
            .map_err(|e| {
                format_local_mysql_connection_error("Gagal terhubung ke MySQL lokal", e)
            })?;

        let rows = sqlx::query(
            "SELECT TABLE_NAME, COALESCE(TABLE_ROWS, 0) AS total_rows 
             FROM information_schema.TABLES 
             WHERE TABLE_SCHEMA = DATABASE() AND TABLE_TYPE = 'BASE TABLE'",
        )
        .fetch_all(&pool)
        .await
        .map_err(|e| format!("Gagal membaca baris tabel lokal: {}", e))?;

        pool.close().await;

        let mut map = HashMap::new();
        for r in rows {
            let name: String = r.try_get("TABLE_NAME").unwrap_or_default();
            let total: i64 = r.try_get("total_rows").unwrap_or(0);
            if !name.is_empty() {
                map.insert(name, total);
            }
        }
        map
    };

    let remote_counts: HashMap<String, i64> = {
        let tunnel = start_ssh_tunnel(ssh_config, remote_db_config)?;
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;

        let tunnel_conn_str = format!(
            "mysql://{}:{}@127.0.0.1:{}/{}",
            urlencoding::encode(&remote_db_config.username),
            urlencoding::encode(&remote_db_config.password),
            tunnel.local_port,
            urlencoding::encode(&remote_db_config.database)
        );

        let pool = MySqlPoolOptions::new()
            .max_connections(2)
            .acquire_timeout(std::time::Duration::from_secs(10))
            .connect(&tunnel_conn_str)
            .await
            .map_err(|e| format!("Gagal terhubung ke remote DB via SSH tunnel: {}", e))?;

        let rows = sqlx::query(
            "SELECT TABLE_NAME, COALESCE(TABLE_ROWS, 0) AS total_rows 
             FROM information_schema.TABLES 
             WHERE TABLE_SCHEMA = DATABASE() AND TABLE_TYPE = 'BASE TABLE' 
             ORDER BY TABLE_NAME ASC",
        )
        .fetch_all(&pool)
        .await
        .map_err(|e| format!("Gagal membaca daftar tabel remote: {}", e))?;

        pool.close().await;
        drop(tunnel);

        let mut map = HashMap::new();
        for r in rows {
            let name: String = r.try_get("TABLE_NAME").unwrap_or_default();
            let total: i64 = r.try_get("total_rows").unwrap_or(0);
            if !name.is_empty() {
                map.insert(name, total);
            }
        }
        map
    };

    let target_names: Vec<String> = match tables {
        Some(list) if !list.is_empty() => list,
        _ => {
            let mut all_set = std::collections::BTreeSet::new();
            for k in remote_counts.keys() {
                all_set.insert(k.clone());
            }
            for k in local_counts.keys() {
                all_set.insert(k.clone());
            }
            all_set.into_iter().collect()
        }
    };

    let mut diffs = Vec::new();
    for name in target_names {
        let rem_opt = remote_counts.get(&name).copied();
        let loc_opt = local_counts.get(&name).copied();

        let (status, remote_rows, local_rows, row_diff) = match (rem_opt, loc_opt) {
            (Some(rem), None) => ("remote_only".to_string(), rem, 0, rem),
            (None, Some(loc)) => ("local_only".to_string(), 0, loc, -loc),
            (Some(rem), Some(loc)) => {
                let diff = rem - loc;
                let st = if diff == 0 {
                    "in_sync".to_string()
                } else {
                    "diverged".to_string()
                };
                (st, rem, loc, diff)
            }
            (None, None) => ("unknown".to_string(), 0, 0, 0),
        };

        diffs.push(TableDiffInfo {
            table_name: name,
            remote_rows,
            local_rows,
            row_diff,
            status,
        });
    }

    Ok(diffs)
}

#[allow(dead_code)]
pub async fn get_table_create_ddl(
    pool: &sqlx::MySqlPool,
    table_name: &str,
) -> Result<Option<String>, String> {
    let exists: Option<String> = sqlx::query_scalar(
        "SELECT TABLE_NAME FROM information_schema.TABLES WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = ? AND TABLE_TYPE = 'BASE TABLE'",
    )
    .bind(table_name)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("Gagal memeriksa tabel lokal '{}': {}", table_name, e))?;

    if exists.is_none() {
        return Ok(None);
    }

    let safe_table = sanitize_identifier(table_name)?;
    let query = format!("SHOW CREATE TABLE {}", safe_table);
    let row = sqlx::query(&query)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("Gagal membaca skema tabel lokal '{}': {}", table_name, e))?;
    let ddl: String = row
        .try_get(1)
        .map_err(|e| format!("Gagal membaca DDL tabel lokal '{}': {}", table_name, e))?;
    Ok(Some(ddl))
}

