use crate::models::{incremental_primary_key, LocalDbConfig};
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
        urlencoding::encode(&config.database)
    )
}

pub(crate) fn format_local_mysql_connection_error(
    context: &str,
    error: impl std::fmt::Display,
) -> String {
    let detail = error.to_string();
    let mut message = format!("{}: {}", context, detail);

    if detail.to_ascii_lowercase().contains("auth_gssapi_client") {
        message.push_str(
            ". Server lokal memakai plugin autentikasi auth_gssapi_client yang belum didukung \
             driver MySQL aplikasi (SQLx). Buat akun MariaDB khusus aplikasi dengan \
             mysql_native_password, beri hak akses ke database lokal, lalu gunakan akun itu \
             pada profil. Contoh: CREATE USER 'db_sync_app'@'127.0.0.1' IDENTIFIED VIA \
             mysql_native_password USING PASSWORD('GANTI_PASSWORD'); GRANT ALL PRIVILEGES ON \
             nama_database.* TO 'db_sync_app'@'127.0.0.1'; Koneksi SSH tidak perlu diubah.",
        );
    }

    message
}

pub fn sanitize_identifier(ident: &str) -> Result<String, String> {
    let trimmed = ident.trim().trim_matches('`');
    if trimmed.is_empty() {
        return Err("Nama tabel atau kolom tidak boleh kosong".to_string());
    }
    let escaped = trimmed.replace('`', "``");
    Ok(format!("`{}`", escaped))
}

fn detect_docker_mysql_cli(container: &str) -> Result<&'static str, String> {
    let mysql_output = crate::process_utils::new_std_command("docker")
        .arg("exec")
        .arg(container)
        .arg("mysql")
        .arg("--version")
        .output()
        .map_err(|e| {
            format!(
                "Gagal menjalankan Docker untuk kontainer '{}': {}. Pastikan Docker terpasang dan aktif.",
                container, e
            )
        })?;
    if mysql_output.status.success() {
        return Ok("mysql");
    }

    let mariadb_output = crate::process_utils::new_std_command("docker")
        .arg("exec")
        .arg(container)
        .arg("mariadb")
        .arg("--version")
        .output()
        .map_err(|e| {
            format!(
                "Kontainer '{}' tidak menjalankan MySQL CLI dan pemeriksaan MariaDB gagal: {}",
                container, e
            )
        })?;
    if mariadb_output.status.success() {
        return Ok("mariadb");
    }

    let output_text = |output: &std::process::Output| {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            format!("exit status {}", output.status)
        }
    };

    Err(format!(
        "Kontainer '{}' tidak memiliki MySQL atau MariaDB CLI.\n• mysql: {}\n• mariadb: {}",
        container,
        output_text(&mysql_output),
        output_text(&mariadb_output)
    ))
}

fn validate_docker_container(config: &LocalDbConfig) -> Result<&str, String> {
    let container = config.docker_container.trim();
    if container.is_empty() {
        return Err("Opsi Docker aktif, tetapi nama container belum diisi.".to_string());
    }
    Ok(container)
}

fn build_create_database_sql(database: &str) -> Result<String, String> {
    let safe_database = sanitize_identifier(database)?;
    Ok(format!(
        "CREATE DATABASE IF NOT EXISTS {} CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;",
        safe_database
    ))
}

async fn create_database_in_docker(
    config: &LocalDbConfig,
    container: &str,
    create_sql: &str,
) -> Result<(), String> {
    let cli = detect_docker_mysql_cli(container)?;
    let mut command = new_tokio_command("docker");
    command
        .arg("exec")
        .arg(container)
        .arg(cli)
        .arg("--skip-ssl")
        .arg("--connect-timeout=60")
        .arg("-u")
        .arg(&config.username);

    if !config.password.is_empty() {
        command.arg(format!("-p{}", config.password));
    }
    command.arg("-e").arg(create_sql);

    let output = command.output().await.map_err(|e| {
        format!(
            "Gagal membuat database lokal via Docker container '{}': {}",
            container, e
        )
    })?;
    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let detail = if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        format!("exit status {}", output.status)
    };
    Err(format!(
        "Gagal membuat database lokal via Docker container '{}': {}",
        container, detail
    ))
}

pub async fn test_local_connection(config: &LocalDbConfig) -> Result<String, String> {
    if config.use_docker {
        let container = validate_docker_container(config)?;
        let cli = detect_docker_mysql_cli(container)?;

        let mut cmd = new_tokio_command("docker");
        cmd.arg("exec")
            .arg(container)
            .arg(cli)
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

        let conn_str = build_local_connection_string(config);
        let tcp_status = match MySqlPoolOptions::new()
            .max_connections(1)
            .acquire_timeout(std::time::Duration::from_secs(2))
            .connect(&conn_str)
            .await
        {
            Ok(p) => {
                p.close().await;
                " [TCP OK]"
            }
            Err(_) => " (Catatan: Port TCP lokal belum terhubung ke host. Pastikan port MySQL di-forward ke host jika sinkronisasi membaca skema/ID)",
        };

        return Ok(format!("Docker [{}] ({}): {}{}", container, cli, version, tcp_status));
    }

    let conn_str = build_local_connection_string(config);
    let pool = MySqlPoolOptions::new()
        .max_connections(2)
        .connect(&conn_str)
        .await
        .map_err(|e| format_local_mysql_connection_error("Gagal terhubung ke MySQL lokal", e))?;

    let row: (String,) = sqlx::query_as("SELECT VERSION()")
        .fetch_one(&pool)
        .await
        .map_err(|e| format!("Gagal mengambil versi MySQL: {}", e))?;

    pool.close().await;
    Ok(row.0)
}

pub async fn ensure_local_database_exists(config: &LocalDbConfig) -> Result<(), String> {
    let create_sql = build_create_database_sql(&config.database)?;

    if config.use_docker {
        let container = validate_docker_container(config)?;
        return create_database_in_docker(config, container, &create_sql).await;
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
        .map_err(|e| {
            format_local_mysql_connection_error("Gagal terhubung ke MySQL server lokal", e)
        })?;

    let database_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM information_schema.SCHEMATA WHERE SCHEMA_NAME = ?",
    )
    .bind(&config.database)
    .fetch_one(&pool)
    .await
    .map_err(|e| format!("Gagal memeriksa database lokal '{}': {}", config.database, e))?;

    if database_count > 0 {
        pool.close().await;
        return Ok(());
    }

    sqlx::query(&create_sql)
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
    let use_docker = local_config.use_docker;
    let container_name = if use_docker {
        validate_docker_container(local_config)?
    } else {
        local_config.docker_container.trim()
    };

    let mut cmd = if use_docker {
        let cli = detect_docker_mysql_cli(container_name)?;
        let mut c = new_tokio_command("docker");
        c.arg("exec")
            .arg("-i")
            .arg(container_name)
            .arg(cli)
            .arg("--force")
            .arg("--skip-ssl")
            .arg("--local-infile=1")
            .arg("--binary-mode")
            .arg("--quick")
            .arg("--max-allowed-packet=1024M")
            .arg("--net-buffer-length=1M")
            .arg("--default-character-set=utf8mb4")
            .arg("--init-command=SET SESSION foreign_key_checks=0; SET SESSION unique_checks=0; SET SESSION autocommit=1; SET SESSION sql_mode='';")
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
            .arg("--force")
            .arg("--skip-ssl")
            .arg("--local-infile=1")
            .arg("--binary-mode")
            .arg("--quick")
            .arg("--max-allowed-packet=1024M")
            .arg("--net-buffer-length=1M")
            .arg("--default-character-set=utf8mb4")
            .arg("--init-command=SET SESSION foreign_key_checks=0; SET SESSION unique_checks=0; SET SESSION autocommit=1; SET SESSION sql_mode='';");

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

    #[cfg(unix)]
    if let Some(ref stdin_ref) = child.stdin {
        crate::process_utils::tune_pipe_capacity(stdin_ref, 512 * 1024);
    }

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
        .map_err(|e| format_local_mysql_connection_error("Koneksi ke MySQL lokal gagal", e))?;

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
        Err(e) => {
            return Err(format_local_mysql_connection_error(
                "Koneksi ke MySQL lokal gagal",
                e,
            ))
        }
    };

    // 1. Get primary keys for tables in the local database
    let pk_query = "
        SELECT TABLE_NAME, COLUMN_NAME, DATA_TYPE, COLUMN_TYPE
        FROM information_schema.COLUMNS
        WHERE TABLE_SCHEMA = DATABASE() AND COLUMN_KEY = 'PRI'
        ORDER BY ORDINAL_POSITION ASC
    ";
    let pk_rows = sqlx::query(pk_query)
        .fetch_all(&pool)
        .await
        .map_err(|e| format!("Gagal membaca metadata primary key lokal: {}", e))?;

    use std::collections::HashMap;
    let mut pk_columns: HashMap<String, Vec<(String, String, String)>> = HashMap::new();
    for r in pk_rows {
        let t_name: String = r.try_get("TABLE_NAME").unwrap_or_default();
        let c_name: String = r.try_get("COLUMN_NAME").unwrap_or_default();
        let data_type: String = r.try_get("DATA_TYPE").unwrap_or_default();
        let column_type: String = r.try_get("COLUMN_TYPE").unwrap_or_default();
        if !t_name.is_empty() && !c_name.is_empty() {
            pk_columns
                .entry(t_name)
                .or_default()
                .push((c_name, data_type, column_type));
        }
    }
    let pk_map: HashMap<String, String> = pk_columns
        .iter()
        .filter_map(|(table, columns)| {
            incremental_primary_key(columns).map(|column| (table.clone(), column))
        })
        .collect();

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
                let q = format!("SELECT CAST(MAX({}) AS CHAR) FROM {}", sc, st);
                if let Ok(row) = sqlx::query(&q).fetch_one(&pool).await {
                    if let Ok(Some(val)) = row.try_get::<Option<String>, _>(0) {
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

pub async fn drop_local_tables(
    config: &LocalDbConfig,
    tables: Vec<String>,
) -> Result<Vec<String>, String> {
    if tables.is_empty() {
        return Ok(Vec::new());
    }

    let conn_str = build_local_connection_string(config);
    let pool = MySqlPoolOptions::new()
        .max_connections(2)
        .acquire_timeout(std::time::Duration::from_secs(6))
        .connect(&conn_str)
        .await
        .map_err(|e| format_local_mysql_connection_error("Koneksi ke MySQL lokal gagal", e))?;

    let mut conn = pool
        .acquire()
        .await
        .map_err(|e| format_local_mysql_connection_error("Koneksi ke MySQL lokal gagal", e))?;

    let _ = sqlx::query("SET FOREIGN_KEY_CHECKS=0").execute(&mut *conn).await;

    let mut dropped = Vec::new();
    for table_name in tables {
        let safe_table = match sanitize_identifier(&table_name) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let query = format!("DROP TABLE IF EXISTS {}", safe_table);
        if let Err(e) = sqlx::query(&query).execute(&mut *conn).await {
            eprintln!("Gagal drop tabel lokal '{}': {}", table_name, e);
        } else {
            dropped.push(table_name);
        }
    }

    let _ = sqlx::query("SET FOREIGN_KEY_CHECKS=1").execute(&mut *conn).await;
    drop(conn);
    pool.close().await;

    Ok(dropped)
}

#[cfg(test)]
mod tests {
    use super::{
        build_create_database_sql, format_local_mysql_connection_error, validate_docker_container,
    };
    use crate::models::LocalDbConfig;

    fn local_config(docker_container: &str) -> LocalDbConfig {
        LocalDbConfig {
            host: "127.0.0.1".to_string(),
            port: 3306,
            username: "root".to_string(),
            password: String::new(),
            database: "sync_db".to_string(),
            use_docker: true,
            docker_container: docker_container.to_string(),
        }
    }

    #[test]
    fn create_database_sql_escapes_database_identifiers() {
        assert_eq!(
            build_create_database_sql("db`name").unwrap(),
            "CREATE DATABASE IF NOT EXISTS `db``name` CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;"
        );
    }

    #[test]
    fn create_database_sql_rejects_empty_database_name() {
        assert!(build_create_database_sql("  ").is_err());
    }

    #[test]
    fn docker_mode_requires_container_name() {
        let error = validate_docker_container(&local_config("  ")).unwrap_err();
        assert!(error.contains("nama container"));
    }

    #[test]
    fn docker_mode_trims_container_name() {
        assert_eq!(
            validate_docker_container(&local_config("  local-mysql  ")).unwrap(),
            "local-mysql"
        );
    }

    #[test]
    fn gssapi_connection_error_explains_supported_local_authentication() {
        let error = format_local_mysql_connection_error(
            "Gagal terhubung ke MySQL lokal",
            "unknown authentication plugin: auth_gssapi_client",
        );

        assert!(error.contains("auth_gssapi_client"));
        assert!(error.contains("mysql_native_password"));
        assert!(error.contains("CREATE USER 'db_sync_app'@'127.0.0.1'"));
        assert!(error.contains("Koneksi SSH tidak perlu diubah"));
    }

    #[test]
    fn other_connection_errors_are_preserved_without_extra_guidance() {
        assert_eq!(
            format_local_mysql_connection_error("Koneksi gagal", "connection refused"),
            "Koneksi gagal: connection refused"
        );
    }
}
