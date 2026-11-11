/**
 * Helper to build sync payload and watermarks for TableView.
 * Complies with AGENTS.md: SoC, SRP (<60 lines).
 */
export function buildTableSyncOptions({
  tableStates,
  tableStatesMap,
  syncMode,
  rowLimitPreset,
  customRowLimit,
  concurrencyWorkers,
}) {
  const selectedItems = [];
  const watermarks = {};

  for (const [tblName, s] of Object.entries(tableStates)) {
    if (s.selected) {
      selectedItems.push({
        name: tblName,
        sync_structure: true,
        sync_data: s.sync_data !== undefined ? Boolean(s.sync_data) : true,
      });

      const st = tableStatesMap[tblName];
      const lastId = st?.lastSyncedId == null ? '' : String(st.lastSyncedId).trim();
      if (/^[0-9]+$/.test(lastId) && /[1-9]/.test(lastId)) {
        watermarks[tblName] = {
          primary_key: st.primaryKey || 'id',
          last_id: lastId,
          last_sync_time: st.lastSyncTime || null,
        };
      }
    }
  }

  const effectiveRowLimit = rowLimitPreset === 'custom'
    ? (customRowLimit > 0 ? customRowLimit : null)
    : (rowLimitPreset > 0 ? rowLimitPreset : null);

  return {
    tables: selectedItems,
    row_limit: effectiveRowLimit,
    sync_mode: syncMode,
    incremental_watermarks: syncMode === 'incremental' ? watermarks : null,
    concurrency: concurrencyWorkers,
  };
}

export function buildQuickSyncOptions(tableName, tableStatesMap) {
  const watermarks = {};
  const st = tableStatesMap[tableName];
  const lastId = st?.lastSyncedId == null ? '' : String(st.lastSyncedId).trim();
  if (/^[0-9]+$/.test(lastId) && /[1-9]/.test(lastId)) {
    watermarks[tableName] = {
      primary_key: st.primaryKey || 'id',
      last_id: lastId,
      last_sync_time: st.lastSyncTime || null,
    };
  }

  return {
    tables: [{ name: tableName, sync_structure: true, sync_data: true }],
    row_limit: null,
    sync_mode: 'incremental',
    incremental_watermarks: watermarks,
    concurrency: 1,
  };
}

