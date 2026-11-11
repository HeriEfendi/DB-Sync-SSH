#[cfg(test)]
mod tests {
    use crate::models::SyncFailure;
    use crate::sync_engine::events::{
        progress_table_counts, resolve_sync_status, table_has_no_data_to_sync,
    };
    use crate::sync_engine::row_counter::MysqlDumpRowCounter;
    use crate::sync_engine::sql_helpers::{
        build_chunk_where_clause, extract_clean_mysql_error, has_mysql_cli_errors,
        is_schema_mismatch_error, normalize_mysql_timestamp,
    };
    use std::collections::HashSet;

    #[test]
    fn mysql_dump_row_counter_handles_newlines_chunks_and_quoted_delimiters() {
        let mut counter = MysqlDumpRowCounter::default();

        counter.push(b"INSERT INTO `items` VALUES\n(1, 'contains ),( inside'),\n");
        counter.push(b"(2, 'another row'); REPLACE INTO `items` VALUES (3, 'three');");

        assert_eq!(counter.total_rows, 3);
    }

    #[test]
    fn empty_incremental_table_is_skipped_but_failure_or_disabled_data_is_not() {
        assert!(table_has_no_data_to_sync(true, 0, false));
        assert!(!table_has_no_data_to_sync(true, 5, false));
        assert!(!table_has_no_data_to_sync(true, 0, true));
        assert!(!table_has_no_data_to_sync(false, 0, false));
    }

    #[test]
    fn progress_counts_unique_failed_and_skipped_tables() {
        let completed = ["customers", "orders", "empty_table"]
            .into_iter()
            .map(str::to_string)
            .collect();
        let skipped = ["empty_table"].into_iter().map(str::to_string).collect();
        let failures = vec![
            SyncFailure {
                table_name: Some("orders".to_string()),
                stage: "Dump SSH".to_string(),
                message: "connection lost".to_string(),
            },
            SyncFailure {
                table_name: Some("orders".to_string()),
                stage: "Importer MySQL lokal".to_string(),
                message: "import failed".to_string(),
            },
            SyncFailure {
                table_name: None,
                stage: "SSH".to_string(),
                message: "connection lost".to_string(),
            },
        ];

        assert_eq!(
            progress_table_counts(&completed, &skipped, &failures),
            (3, 1, 1, 1)
        );
        assert_eq!(
            progress_table_counts(&HashSet::new(), &HashSet::new(), &failures),
            (1, 0, 1, 0)
        );
    }

    #[test]
    fn mysql_cli_errors_are_detected_without_treating_warnings_as_failures() {
        assert!(has_mysql_cli_errors(
            "ERROR 1054 (42S22) at line 10: Unknown column"
        ));
        assert!(has_mysql_cli_errors(" ERROR: connection lost"));
        assert!(!has_mysql_cli_errors("Warning: using a password on the command line"));
        assert!(!has_mysql_cli_errors(""));
    }

    #[test]
    fn sync_status_is_success_only_when_everything_finishes_cleanly() {
        assert_eq!(resolve_sync_status(false, false, true, false), "success");
    }

    #[test]
    fn sync_status_is_partial_when_all_tables_finish_with_table_errors() {
        assert_eq!(resolve_sync_status(false, false, true, true), "partial");
    }

    #[test]
    fn sync_status_is_failed_when_pipeline_stops_before_all_tables() {
        assert_eq!(resolve_sync_status(false, false, false, true), "failed");
    }

    #[test]
    fn sync_status_is_failed_for_a_fatal_importer_error() {
        assert_eq!(resolve_sync_status(false, true, true, true), "failed");
    }

    #[test]
    fn sync_status_keeps_cancellation_distinct_from_failure() {
        assert_eq!(resolve_sync_status(true, true, false, true), "cancelled");
    }

    #[test]
    fn mysql_watermark_timestamp_accepts_remote_datetime_format() {
        assert_eq!(
            normalize_mysql_timestamp("2026-06-02 13:14:15.123456").as_deref(),
            Some("2026-06-02 13:14:15.123456")
        );
    }

    #[test]
    fn mysql_watermark_timestamp_rejects_local_iso_timestamp() {
        assert_eq!(normalize_mysql_timestamp("2026-06-02T13:14:15.123Z"), None);
    }

    #[test]
    fn incremental_chunk_filter_includes_updated_rows_below_id_watermark() {
        let clause = build_chunk_where_clause(
            "`id`",
            None,
            500,
            Some((900, "`updated_at`", "2026-06-02 13:14:15.123456")),
        );

        assert_eq!(
            clause,
            "--where=(`id` > 900 OR `updated_at` >= '2026-06-02 13:14:15.123456') AND `id` <= 500"
        );
    }

    #[test]
    fn incremental_chunk_filter_preserves_keyset_range_without_timestamp_watermark() {
        assert_eq!(
            build_chunk_where_clause("`id`", Some(100), 500, None),
            "--where=`id` > 100 AND `id` <= 500"
        );
    }

    #[test]
    fn first_chunk_includes_zero_and_negative_primary_keys() {
        assert_eq!(
            build_chunk_where_clause("`id`", None, 500, None),
            "--where=`id` <= 500"
        );
    }

    #[test]
    fn schema_mismatch_detection_identifies_structure_errors_and_extracts_clean_line() {
        let err1 = "ERROR 1054 (42S22) at line 12: Unknown column 'new_col' in 'field list'";
        assert!(is_schema_mismatch_error(err1));
        assert_eq!(extract_clean_mysql_error(err1), err1);

        let err2 = "ERROR 1146 (42S02): Table 'test.not_exists' doesn't exist";
        assert!(is_schema_mismatch_error(err2));
        assert_eq!(extract_clean_mysql_error(err2), err2);

        let generic_err = "ERROR 2006 (HY000): MySQL server has gone away";
        assert!(!is_schema_mismatch_error(generic_err));
        assert_eq!(extract_clean_mysql_error(generic_err), generic_err);
    }
}

