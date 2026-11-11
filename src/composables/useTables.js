import { ref } from 'vue';
import { getAllTableStates, saveTableState } from '../services/syncStateStore.js';
import { safeInvoke } from '../services/tauriHelper.js';

/**
 * Composable for managing remote table inspection and local sync states.
 * Complies with AGENTS.md: Separation of Concerns (UI vs Business Logic).
 */
export function useTables() {
  const tables = ref([]);
  const tableStatesMap = ref({});
  const isLoadingTables = ref(false);

  function refreshLocalTableStates(profileId) {
    if (!profileId) return;
    tableStatesMap.value = getAllTableStates(profileId);
  }

  async function detectAndSyncLocalIds(profile, tableList = null) {
    if (!profile?.local_db) return;
    const profileId = profile.id;
    try {
      const targetNames = tableList || tables.value.map((t) => t.name);
      if (!targetNames || targetNames.length === 0) return;

      const localInfos = await safeInvoke('get_all_tables_last_local_ids', {
        config: profile.local_db,
        tables: targetNames,
      });

      if (Array.isArray(localInfos)) {
        const latestStates = getAllTableStates(profileId);
        for (const info of localInfos) {
          const existing = latestStates[info.table_name] || {};
          const updated = {
            ...existing,
            primaryKey: info.primary_key,
            lastSyncedId: info.max_id_str ?? info.max_id,
          };
          saveTableState(profileId, info.table_name, updated);
        }
        refreshLocalTableStates(profileId);
      }
    } catch (err) {
      console.warn('Gagal mendeteksi last local IDs:', err);
    }
  }

  async function fetchTables(profile) {
    if (!profile) return;
    refreshLocalTableStates(profile.id);
    isLoadingTables.value = true;
    try {
      const list = await safeInvoke('get_remote_tables', {
        sshConfig: profile.ssh,
        remoteDbConfig: profile.remote_db,
      });
      tables.value = Array.isArray(list) ? list : [];
      await detectAndSyncLocalIds(profile);
    } catch (err) {
      console.error('Gagal mengambil daftar tabel remote:', err);
    } finally {
      isLoadingTables.value = false;
    }
  }

  return {
    tables,
    tableStatesMap,
    isLoadingTables,
    refreshLocalTableStates,
    detectAndSyncLocalIds,
    fetchTables,
  };
}

