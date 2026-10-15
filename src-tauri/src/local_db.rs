use crate::models::LocalDbConfig;
use crate::process_utils::new_tokio_command;
use sqlx::{mysql::MySqlPoolOptions, Row};
use std::process::Stdio;

pub fn build_local_connection_string(config: &LocalDbConfig) -> String {
    let host = if config.host.trim().is_empty() {
        "127.0.0.1"
    } else {
        config.host.trim()
    };
    let port = if config.port == 0 { 3306 } else { config.port };

    format!(
        "mysql://{}:{}@{}:{}/{}",
        urlencoding::encode(&config.username),
        urlencoding::encode(&config.password),
        host,
        port,
        config.database
    )
}

pub fn sanitize_identifier(ident: &str) -> Result<String, String> {
    let trimmed = ident.trim().trim_matches('`');
    if trimmed.is_empty() {
        return Err("Nama tabel atau kolom tidak boleh kosong".to_string());
    }
    let escaped = trimmed.replace('`', "``");
    Ok(format!("`{}`", escaped))
}

pub async fn test_local_connection(config: &LocalDbConfig) -> Result<String, String> {
    if config.use_docker {
        let container = config.docker_container.trim();
        if container.is_empty() {
            return Err("Nama Docker container belum diisi".to_string());
        }

        let mut cmd = new_tokio_command("docker");
        cmd.arg("exec")
            .arg(container)
            .arg("mysql")
            .arg("-u")
            .arg(&config.username);

        if !config.password.is_empty() {
            cmd.arg(format!("-p{}", config.password));
        }

        cmd.arg("-e").arg("SELECT VERSION()");

        let output = cmd
            .output()
            .await
            .map_err(|e| format!("Gagal mengeksekusi docker exec mysql: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Docker MySQL error: {}", stderr));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let version = stdout
            .lines()
            .nth(1)
            .unwrap_or("Docker MySQL Connected")
            .trim();
        return Ok(format!("Docker [{}]: {}", container, version));
    }

    let conn_str = build_local_connection_string(config);
    let pool = MySqlPoolOptions::new()
        .max_connections(2)
        .connect(&conn_str)
        .await
        .map_err(|e| format!("Gagal terhubung ke MySQL lokal: {}", e))?;

    let row: (String,) = sqlx::query_as("SELECT VERSION()")
        .fetch_one(&pool)
        .await
        .map_err(|e| format!("Gagal mengambil versi MySQL: {}", e))?;

    pool.close().await;
    Ok(row.0)
}

pub async fn ensure_local_database_exists(config: &LocalDbConfig) -> Result<(), String> {
    if config.database.trim().is_empty() {
        return Err("Nama database lokal tidak boleh kosong".to_string());
    }

    let host = if config.host.trim().is_empty() {
        "127.0.0.1"
    } else {
        config.host.trim()
    };
    let port = if config.port == 0 { 3306 } else { config.port };

    // Connect to MySQL server root/without database to check/create database
    let root_conn_str = format!(
        "mysql://{}:{}@{}:{}",
        urlencoding::encode(&config.username),
        urlencoding::encode(&config.password),
        host,
        port
    );

    let pool = MySqlPoolOptions::new()
        .max_connections(2)
        .connect(&root_conn_str)
        .await
        .map_err(|e| format!("Gagal terhubung ke MySQL server lokal: {}", e))?;

    let safe_db = sanitize_identifier(&config.database)?;
    let query = format!(
        "CREATE DATABASE IF NOT EXISTS {} CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci",
        safe_db
    );

    sqlx::query(&query)
        .execute(&pool)
        .await
        .map_err(|e| format!("Gagal membuat database lokal '{}': {}", config.database, e))?;

    pool.close().await;
    Ok(())
}

pub fn spawn_mysql_child(
    local_config: &LocalDbConfig,
) -> Result<
    (
        tokio::process::Child,
        tokio::process::ChildStdin,
        tokio::process::ChildStderr,
    ),
    String,
> {
    let use_docker = local_config.use_docker && !local_config.docker_container.trim().is_empty();
    let container_name = local_config.docker_container.trim();

    let mut cmd = if use_docker {
        let mut c = new_tokio_command("docker");
        c.arg("exec")
            .arg("-i")
            .arg(container_name)
            .arg("mysql")
            .arg("--skip-ssl")
            .arg("--local-infile=1")
            .arg("--quick")
            .arg("--max-allowed-packet=1024M")
            .arg("-u")
            .arg(&local_config.username);

        if !local_config.password.is_empty() {
            c.arg(format!("-p{}", local_config.password));
        }

        c.arg(&local_config.database);
        c
    } else {
        let host = if local_config.host.trim().is_empty() {
            "127.0.0.1"
        } else {
            local_config.host.trim()
        };
        let port = if local_config.port == 0 {
            3306
        } else {
            local_config.port
        };

        // Check if mariadb or mysql cli binary is available
        let binary = if std::path::Path::new("/usr/bin/mariadb").exists() {
            "mariadb"
        } else {
            "mysql"
        };

        let mut c = new_tokio_command(binary);
        c.arg("-h")
            .arg(host)
            .arg("-P")
            .arg(port.to_string())
            .arg("-u")
            .arg(&local_config.username)
            .arg("--skip-ssl")
            .arg("--local-infile=1")
            .arg("--quick")
            .arg("--max-allowed-packet=1024M")
            .arg("--default-character-set=utf8mb4");

        if !local_config.password.is_empty() {
            c.arg(format!("-p{}", local_config.password));
        }

        c.arg(&local_config.database);
        c
    };

    cmd.stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Gagal menjalankan proses import MySQL lokal: {}", e))?;

    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| "Gagal mengambil STDIN proses MySQL".to_string())?;

    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Gagal mengambil STDERR proses MySQL".to_string())?;

    Ok((child, stdin, stderr))
}

pub async fn get_local_tables(config: &LocalDbConfig) -> Result<Vec<String>, String> {
    let conn_str = build_local_connection_string(config);
    let pool = MySqlPoolOptions::new()
        .max_connections(2)
        .connect(&conn_str)
        .await
        .map_err(|e| format!("Koneksi ke MySQL lokal gagal: {}", e))?;

    let rows = sqlx::query("SELECT TABLE_NAME FROM information_schema.TABLES WHERE TABLE_SCHEMA = DATABASE() AND TABLE_TYPE = 'BASE TABLE' ORDER BY TABLE_NAME ASC")
        .fetch_all(&pool)
        .await
        .map_err(|e| format!("Gagal membaca daftar tabel lokal: {}", e))?;

    pool.close().await;

    let mut tables = Vec::new();
    for row in rows {
        if let Ok(name) = row.try_get::<String, _>(0) {
            tables.push(name);
        }
    }
    Ok(tables)
}

pub async fn get_all_tables_last_local_ids(
    config: &LocalDbConfig,
    tables: Option<Vec<String>>,
) -> Result<Vec<crate::models::TableLastIdInfo>, String> {
    let conn_str = build_local_connection_string(config);
    let pool = match MySqlPoolOptions::new()
        .max_connections(2)
        .acquire_timeout(std::time::Duration::from_secs(6))
        .connect(&conn_str)
        .await
    {
        Ok(p) => p,
        Err(e) => return Err(format!("Koneksi ke MySQL lokal gagal: {}", e)),
    };

    // 1. Get primary keys for tables in the local database
    let pk_query = "
        SELECT TABLE_NAME, COLUMN_NAME, DATA_TYPE
        FROM information_schema.COLUMNS
        WHERE TABLE_SCHEMA = DATABASE() AND COLUMN_KEY = 'PRI'
        ORDER BY ORDINAL_POSITION ASC
    ";
    let pk_rows = sqlx::query(pk_query)
        .fetch_all(&pool)
        .await
        .unwrap_or_default();

    use std::collections::HashMap;
    let mut pk_map: HashMap<String, String> = HashMap::new();
    for r in pk_rows {
        let t_name: String = r.try_get("TABLE_NAME").unwrap_or_default();
        let c_name: String = r.try_get("COLUMN_NAME").unwrap_or_default();
        if !t_name.is_empty() && !c_name.is_empty() && !pk_map.contains_key(&t_name) {
            pk_map.insert(t_name, c_name);
        }
    }

    // 2. Target tables list
    let target_tables = match tables {
        Some(list) if !list.is_empty() => list,
        _ => {
            let mut all = Vec::new();
            if let Ok(t_rows) = sqlx::query(
                "SELECT TABLE_NAME FROM information_schema.TABLES WHERE TABLE_SCHEMA = DATABASE() AND TABLE_TYPE = 'BASE TABLE' ORDER BY TABLE_NAME ASC"
            )
            .fetch_all(&pool)
            .await
            {
                for tr in t_rows {
                    if let Ok(name) = tr.try_get::<String, _>(0) {
                        all.push(name);
                    }
                }
            }
            all
        }
    };

    let mut results = Vec::new();
    for table in target_tables {
        let pk_opt = pk_map.get(&table).cloned();
        let mut max_id: Option<i64> = None;
        let mut max_id_str: Option<String> = None;

        if let Some(ref pk) = pk_opt {
            let safe_t = sanitize_identifier(&table);
            let safe_c = sanitize_identifier(pk);
            if let (Ok(st), Ok(sc)) = (safe_t, safe_c) {
                let q = format!("SELECT MAX({}) FROM {}", sc, st);
                if let Ok(row) = sqlx::query(&q).fetch_one(&pool).await {
                    if let Ok(val) = row.try_get::<i64, _>(0) {
                        max_id = Some(val);
                        max_id_str = Some(val.to_string());
                    } else if let Ok(val) = row.try_get::<u64, _>(0) {
                        max_id = Some(val as i64);
                        max_id_str = Some(val.to_string());
                    } else if let Ok(val) = row.try_get::<i32, _>(0) {
                        max_id = Some(val as i64);
                        max_id_str = Some(val.to_string());
                    } else if let Ok(val) = row.try_get::<String, _>(0) {
                        max_id = val.parse::<i64>().ok();
                        max_id_str = Some(val);
                    }
                }
            }
        }

        results.push(crate::models::TableLastIdInfo {
            table_name: table,
            primary_key: pk_opt,
            max_id,
            max_id_str,
        });
    }

    pool.close().await;
    Ok(results)
}
