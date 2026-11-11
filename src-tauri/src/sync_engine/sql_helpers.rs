pub fn has_mysql_cli_errors(stderr: &str) -> bool {
    stderr.lines().any(|line| {
        let line = line.trim_start();
        line.starts_with("ERROR ") || line.starts_with("ERROR:")
    })
}

pub fn is_schema_mismatch_error(err: &str) -> bool {
    let lower = err.to_lowercase();
    lower.contains("unknown column")
        || lower.contains("column count doesn't match")
        || (lower.contains("table") && lower.contains("doesn't exist"))
        || lower.contains("doesn't have a default value")
        || lower.contains("data truncated")
        || lower.contains("cannot be null")
        || lower.contains("error 1054")
        || lower.contains("error 1136")
        || lower.contains("error 1146")
        || lower.contains("error 1364")
        || lower.contains("error 1265")
        || lower.contains("error 1048")
        || lower.contains("incorrect integer value")
        || lower.contains("incorrect decimal value")
        || lower.contains("incorrect date")
        || lower.contains("error 1366")
        || lower.contains("error 1292")
}

pub fn extract_clean_mysql_error(err: &str) -> String {
    for line in err.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("ERROR ")
            || trimmed.starts_with("ERROR:")
            || trimmed.contains("Unknown column")
            || trimmed.contains("doesn't exist")
        {
            return trimmed.to_string();
        }
    }
    err.lines().next().unwrap_or(err).trim().to_string()
}

pub fn normalize_mysql_timestamp(value: &str) -> Option<String> {
    chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S%.f")
        .ok()
        .map(|timestamp| timestamp.format("%Y-%m-%d %H:%M:%S%.f").to_string())
}

#[allow(dead_code)]
pub fn normalize_create_table_ddl(ddl: &str) -> String {
    let body = match (ddl.find('('), ddl.rfind(')')) {
        (Some(start), Some(end)) if start < end => &ddl[start + 1..end],
        _ => ddl,
    };

    let mut clean_body = String::with_capacity(body.len());
    for c in body.chars() {
        if c != '`' {
            clean_body.push(c.to_ascii_lowercase());
        }
    }

    let tokens: Vec<&str> = clean_body.split_whitespace().collect();
    let mut normalized_tokens = Vec::with_capacity(tokens.len());

    for token in tokens {
        let clean = if let Some(idx) = token.find('(') {
            let prefix = &token[..idx];
            if matches!(
                prefix,
                "int" | "tinyint" | "smallint" | "mediumint" | "bigint"
            ) {
                if token.ends_with(',') {
                    format!("{},", prefix)
                } else {
                    prefix.to_string()
                }
            } else {
                token.to_string()
            }
        } else {
            token.to_string()
        };
        normalized_tokens.push(clean);
    }

    normalized_tokens.join(" ")
}

pub fn build_chunk_where_clause(
    safe_pk: &str,
    current_last_id: Option<i128>,
    chunk_max: i128,
    update_filter: Option<(i128, &str, &str)>,
) -> String {
    let range_condition = match current_last_id {
        Some(last_id) => format!("{} > {} AND {} <= {}", safe_pk, last_id, safe_pk, chunk_max),
        None => format!("{} <= {}", safe_pk, chunk_max),
    };
    let condition = match update_filter {
        Some((watermark_id, updated_at_col, timestamp)) => format!(
            "({} > {} OR {} >= '{}') AND {}",
            safe_pk, watermark_id, updated_at_col, timestamp, range_condition
        ),
        None => range_condition,
    };
    format!("--where={}", condition)
}

pub fn format_number(n: usize) -> String {
    let s = n.to_string();
    let mut result = String::new();
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    for (i, c) in chars.iter().enumerate() {
        result.push(*c);
        let rem = len - 1 - i;
        if rem > 0 && rem % 3 == 0 {
            result.push('.');
        }
    }
    result
}

static DUMP_COMPRESSION_ARGS: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();

pub fn get_dump_compression_args(dump_bin: &str) -> &'static [String] {
    DUMP_COMPRESSION_ARGS.get_or_init(|| {
        let mut check_cmd = crate::process_utils::new_std_command(dump_bin);
        check_cmd.arg("--help");
        if let Ok(output) = check_cmd.output() {
            let help_text = String::from_utf8_lossy(&output.stdout);
            if help_text.contains("--compression-algorithms") {
                return vec![
                    "--compression-algorithms=zstd,zlib,uncompressed".to_string(),
                    "--zstd-compression-level=1".to_string(),
                ];
            }
        }
        vec!["--compress".to_string()]
    })
}
