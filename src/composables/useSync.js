import { ref } from 'vue';
import { safeInvoke, safeListen } from '../services/tauriHelper.js';
import { updateAppletProgress } from '../services/appletProgress.js';
import { saveTableState, getAllTableStates } from '../services/syncStateStore.js';
import { formatDurationMs, extractTableName, historyStatus } from '../utils/formatters.js';

const INITIAL_PROGRESS = {
  current_table_index: 0,
  total_tables: 0,
  completed_tables: 0,
  completed_table_names: [],
  skipped_table_names: [],
  current_table_name: '',
  rows_synced_current_table: 0,
  total_synced_all_tables: 0,
  processed_tables: 0,
  successful_tables: 0,
  failed_tables: 0,
  skipped_tables: 0,
  elapsed_ms: 0,
  status: 'idle',
};

/**
 * Composable for managing SSH database synchronization tasks and event listeners.
 * Complies with AGENTS.md: SoC, SRP, modularization (<250 lines).
 */
export function useSync() {
  const isSyncing = ref(false);
  const showSyncModal = ref(false);
  const syncStartedAt = ref(0);
  const syncLogs = ref([]);
  const syncResult = ref(null);
  const isCancelling = ref(false);
  const syncProgress = ref({ ...INITIAL_PROGRESS });

  let unlisteners = [];

  function appendSyncLog(log) {
    if (
      log.details &&
      syncLogs.value.some(
        (existing) =>
          existing.message === log.message &&
          existing.table_name === log.table_name &&
          existing.details === log.details
      )
    ) {
      return;
    }
    syncLogs.value.push(log);
  }

  async function setupEventListeners() {
    try {
      const unLog = await safeListen('sync-log', (event) => {
        if (event.payload) appendSyncLog(event.payload);
      });
      unlisteners.push(unLog);

      const unProgress = await safeListen('sync-progress', (event) => {
        const payload = event.payload;
        if (payload) {
          syncProgress.value = payload;
          const total = payload.total_tables || 0;
          const processed = payload.processed_tables ?? payload.completed_tables ?? 0;
          const pct = total > 0 ? Math.round((processed / total) * 100) : 0;
          const label = `${payload.current_table_index || 0}/${total} (${payload.current_table_name || 'Menyiapkan...'})`;
          updateAppletProgress(pct, label);
        }
      });
      unlisteners.push(unProgress);
    } catch (e) {
      console.warn('Failed to register sync listeners:', e);
    }
  }

  function cleanupEventListeners() {
    unlisteners.forEach((fn) => typeof fn === 'function' && fn());
    unlisteners = [];
  }

  async function startSync(syncOptions, activeProfile, { onHistoryRecord, onAfterSync }) {
    if (isSyncing.value || !activeProfile) return;

    isSyncing.value = true;
    showSyncModal.value = true;
    syncStartedAt.value = Date.now();
    syncLogs.value = [];
    syncResult.value = null;

    const syncingProfile = activeProfile;
    const selectedTables = syncOptions.tables.map((t) => t.name);

    syncProgress.value = {
      ...INITIAL_PROGRESS,
      total_tables: syncOptions.tables.length,
      current_table_name: syncOptions.tables[0]?.name || '',
      status: 'starting',
    };

    try {
      const res = await safeInvoke('start_ssh_sync', {
        sshConfig: syncingProfile.ssh,
        remoteDbConfig: syncingProfile.remote_db,
        localDbConfig: syncingProfile.local_db,
        syncOptions: {
          tables: syncOptions.tables,
          row_limit: syncOptions.row_limit,
          throttle_ms: 60,
          sync_mode: syncOptions.sync_mode || 'incremental',
          incremental_watermarks: syncOptions.incremental_watermarks || null,
          concurrency: syncOptions.concurrency || 2,
        },
      });

      if (res) {
        const status = res.status || (res.success ? 'success' : 'failed');
        syncResult.value = { ...res, status };
        syncProgress.value = {
          ...syncProgress.value,
          current_table_name: '',
          rows_synced_current_table: 0,
          processed_tables: res.processed_tables ?? syncProgress.value.processed_tables,
          successful_tables: res.successful_tables ?? syncProgress.value.successful_tables,
          failed_tables: res.failed_tables ?? syncProgress.value.failed_tables,
          skipped_tables: res.skipped_tables ?? syncProgress.value.skipped_tables,
          elapsed_ms: res.duration_ms ?? syncProgress.value.elapsed_ms,
          status,
        };

        for (const failure of res.failures || []) {
          const tablePrefix = failure.table_name ? `[Tabel '${failure.table_name}'] ` : '';
          appendSyncLog({
            type: 'error',
            table_name: failure.table_name,
            message: `${tablePrefix}${failure.stage} gagal`,
            details: failure.message,
            timestamp: new Date().toLocaleTimeString(),
          });
        }

        const durText = formatDurationMs(res.duration_ms || 0);
        const failedTableNames = [
          ...new Set((res.failures || []).map((f) => f.table_name).filter(Boolean)),
        ];

        const structureChangedTables = [
          ...new Set(
            (res.schema_drift_tables || []).concat(
              syncLogs.value
                .filter((l) =>
                  l.message && (
                    l.message.includes('perubahan struktur') ||
                    l.message.includes('skema lokal tidak selaras')
                  )
                )
                .map((l) => l.table_name || extractTableName(l.message))
                .filter(Boolean)
            )
          ),
        ];

        if (onHistoryRecord) {
          onHistoryRecord({
            profile_name: syncingProfile.name || 'database',
            profile_id: syncingProfile.id,
            status: historyStatus(status),
            tables_count: res.total_tables || 0,
            rows_copied: res.total_rows_synced || 0,
            duration_text: durText,
            failed_tables: failedTableNames,
            structure_changed_tables: structureChangedTables,
            failures: res.failures || [],
            mode: syncOptions.sync_mode || 'incremental',
            row_limit: syncOptions.row_limit,
            logs: syncLogs.value.filter((l) => l.message && l.message.trim() && l.type !== 'info'),
          });
        }

        if ('Notification' in window && Notification.permission === 'granted') {
          const notifBody = status === 'success'
            ? `Sinkronisasi '${syncingProfile.name}' selesai (${res.total_tables || 0} tabel, ${res.total_rows_synced || 0} baris).`
            : `Sinkronisasi '${syncingProfile.name}' ${status} (${res.total_rows_synced || 0} baris tersalin).`;
          new Notification('DB-Sync-Ssh', { body: notifBody });
        }

        if (status === 'success' || status === 'partial') {
          const failedSet = new Set(failedTableNames);
          const successfulTables = selectedTables.filter((t) => !failedSet.has(t));

          if (successfulTables.length > 0) {
            const now = Date.now();
            const lastSyncTime = res.sync_timestamp || new Date(now).toISOString();
            const returnedEpoch = Number(res.sync_timestamp_epoch_ms);
            const lastSyncTimeEpochMs =
              res.sync_timestamp_epoch_ms != null && Number.isFinite(returnedEpoch)
                ? returnedEpoch
                : now;

            for (const tableName of successfulTables) {
              saveTableState(syncingProfile.id, tableName, {
                lastSyncTime,
                lastSyncTimeEpochMs,
              });
            }

            if (onAfterSync) {
              await onAfterSync(successfulTables);
            }
          }
        }
      }
    } catch (err) {
      console.error('Sinkronisasi error:', err);
      const elapsed = Date.now() - syncStartedAt.value;
      syncResult.value = { status: 'failed', success: false, error: String(err), duration_ms: elapsed };
      syncProgress.value = { ...syncProgress.value, elapsed_ms: elapsed, status: 'failed' };
      appendSyncLog({
        type: 'error',
        message: `Error: ${err}`,
        timestamp: new Date().toLocaleTimeString(),
        details: String(err),
      });

      if (onHistoryRecord) {
        onHistoryRecord({
          profile_name: syncingProfile.name || 'database',
          profile_id: syncingProfile.id,
          status: 'Failed',
          tables_count: selectedTables.length,
          rows_copied: 0,
          duration_text: '—',
          failed_tables: [],
          structure_changed_tables: [],
          failures: [{ stage: 'Koneksi / Sync', message: String(err) }],
          mode: syncOptions.sync_mode || 'incremental',
          row_limit: syncOptions.row_limit,
          logs: syncLogs.value.filter((l) => l.message && l.message.trim() && l.type !== 'info'),
        });
      }
    } finally {
      isSyncing.value = false;
      isCancelling.value = false;
    }
  }

  async function stopSync() {
    if (isCancelling.value) return;
    isCancelling.value = true;
    try {
      await safeInvoke('cancel_ssh_sync');
    } catch (err) {
      console.warn('Gagal membatalkan sinkronisasi:', err);
      isCancelling.value = false;
    }
  }

  return {
    isSyncing,
    showSyncModal,
    syncStartedAt,
    syncLogs,
    syncResult,
    isCancelling,
    syncProgress,
    setupEventListeners,
    cleanupEventListeners,
    startSync,
    stopSync,
  };
}

