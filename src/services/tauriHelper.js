/**
 * tauriHelper.js
 * Safe wrappers for Tauri 2.0 IPC with web browser fallback
 */

export function isTauriEnvironment() {
  return (
    typeof window !== 'undefined' &&
    Boolean(
      window.__TAURI_INTERNALS__ ||
      window.__TAURI__ ||
      (window.__TAURI_METADATA__ && window.__TAURI_METADATA__.__currentWindow)
    )
  );
}

export async function safeInvoke(cmd, args = {}) {
  if (isTauriEnvironment()) {
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      return await invoke(cmd, args);
    } catch (err) {
      console.error(`[Tauri IPC] Error invoking '${cmd}':`, err);
      throw err;
    }
  }

  console.warn(`[Tauri IPC Fallback] Mocking '${cmd}' in non-Tauri environment:`, args);

  // Browser mock responses for dev/preview
  if (cmd === 'test_ssh_and_remote_db') {
    return 'SSH OK & Remote MySQL OK (10.6.18-MariaDB-log)';
  }
  if (cmd === 'test_local_connection') {
    return '10.11.8-MariaDB (Local native client)';
  }
  if (cmd === 'get_remote_tables') {
    return [
      { name: 'accounting_asset_categories', total_rows: 13, data_size_bytes: 16384 },
      { name: 'accounting_benchmark_price_import_histories', total_rows: 19, data_size_bytes: 32768 },
      { name: 'accounting_cash_bank_reconciliations', total_rows: 77, data_size_bytes: 65536 },
      { name: 'accounting_cash_bank_reconciliations_approvals', total_rows: 15, data_size_bytes: 16384 },
      { name: 'accounting_cash_bank_reconciliations_approval_eligibilities', total_rows: 0, data_size_bytes: 16384 },
      { name: 'accounting_cash_bank_reconciliation_details', total_rows: 135, data_size_bytes: 131072 },
      { name: 'accounting_cash_bank_reconciliation_detail_imports', total_rows: 905, data_size_bytes: 524288 },
      { name: 'accounting_cash_bank_reconciliation_export_files', total_rows: 2, data_size_bytes: 16384 },
      { name: 'accounting_cash_bank_reconciliation_export_temps', total_rows: 0, data_size_bytes: 16384 },
      { name: 'accounting_cash_bank_reconciliation_import_logs', total_rows: 0, data_size_bytes: 16384 },
      { name: 'accounting_chart_of_accounts', total_rows: 14326, data_size_bytes: 2097152 },
      { name: 'customers', total_rows: 4520, data_size_bytes: 1048576 },
      { name: 'orders', total_rows: 125890, data_size_bytes: 16777216 },
      { name: 'order_items', total_rows: 389400, data_size_bytes: 33554432 },
      { name: 'products', total_rows: 8420, data_size_bytes: 2097152 },
      { name: 'users', total_rows: 320, data_size_bytes: 65536 },
    ];
  }
  if (cmd === 'get_local_tables') {
    return ['accounting_asset_categories', 'customers', 'products'];
  }
  if (cmd === 'get_all_tables_last_local_ids') {
    return [
      { table_name: 'customers', primary_key: 'id', max_id: 4210, max_id_str: '4210' },
      { table_name: 'orders', primary_key: 'id', max_id: 120500, max_id_str: '120500' },
      { table_name: 'products', primary_key: 'id', max_id: 8420, max_id_str: '8420' },
    ];
  }
  if (cmd === 'pick_private_key_file') {
    return '/home/lenovo/.ssh/id_rsa';
  }
  if (cmd === 'start_ssh_sync') {
    return {
      success: true,
      total_tables: args.sync_options?.tables?.length || 10,
      total_rows_synced: 582012,
      duration_ms: 139000,
      error: null,
    };
  }
  if (cmd === 'cancel_ssh_sync') {
    return null;
  }

  return null;
}

export async function safeListen(event, handler) {
  if (isTauriEnvironment()) {
    try {
      const { listen } = await import('@tauri-apps/api/event');
      return await listen(event, handler);
    } catch (err) {
      console.warn(`[Tauri Event] Failed to listen to '${event}':`, err);
      return () => {};
    }
  }

  return () => {};
}
