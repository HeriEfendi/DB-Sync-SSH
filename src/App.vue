<template>
  <div class="app-container">
    <!-- Left Navigation Sidebar -->
    <Sidebar
      :profiles="profiles"
      :active-profile-id="activeProfileId"
      :current-view="currentView"
      @select-profile="handleSelectProfile"
      @create-profile="handleCreateProfile"
      @edit-profile="handleEditProfile"
      @delete-profile="handleDeleteProfile"
      @open-history="currentView = 'history'"
    />

    <!-- Main Panel Content -->
    <main class="main-panel">
      <!-- 1. Table View (Screenshot 1) -->
      <TableView
        v-if="currentView === 'tables' && activeProfile"
        :profile="activeProfile"
        :tables="tables"
        :table-states-map="tableStatesMap"
        :is-loading-tables="isLoadingTables"
        :is-syncing="isSyncing"
        @refresh-tables="fetchTables"
        @start-sync="handleStartSync"
        @save-template="handleSaveTemplate"
        @update-template="handleUpdateTemplate"
        @edit-profile="handleEditProfile"
      />

      <!-- 2. Profile Form View (Screenshot 2) -->
      <ProfileForm
        v-else-if="currentView === 'new-profile' || currentView === 'edit-profile'"
        :initial-profile="currentView === 'edit-profile' ? activeProfile : null"
        :is-editing="currentView === 'edit-profile'"
        @save="handleSaveProfile"
        @cancel="currentView = 'tables'"
        @delete="handleDeleteProfile"
      />

      <!-- 3. History View (Screenshot 3) -->
      <HistoryView
        v-else-if="currentView === 'history'"
        :history-list="historyList"
        @clear-history="handleClearHistory"
        @rerun-sync="handleRerunSync"
      />
    </main>

    <!-- Live Sync Modal with Real-time Logs -->
    <SyncProgressModal
      v-if="showSyncModal"
      :profile-name="activeProfile?.name || ''"
      :progress="syncProgress"
      :logs="syncLogs"
      :started-at="syncStartedAt"
      :is-finished="!isSyncing"
      :result="syncResult"
      :is-cancelling="isCancelling"
      @stop-sync="handleStopSync"
      @minimize="showSyncModal = false"
      @close="showSyncModal = false"
    />

    <!-- Unobtrusive Floating Progress Widget when Modal Minimized -->
    <MiniFloatingSyncWidget
      v-if="!showSyncModal && (isSyncing || syncResult)"
      :profile-name="activeProfile?.name || ''"
      :progress="syncProgress"
      :is-syncing="isSyncing"
      :is-cancelling="isCancelling"
      :result="syncResult"
      :started-at="syncStartedAt"
      @expand="showSyncModal = true"
      @stop-sync="handleStopSync"
      @dismiss="syncResult = null"
    />
  </div>
</template>

<script setup>
import { ref, computed, onMounted, onBeforeUnmount } from 'vue';
import Sidebar from './components/Sidebar.vue';
import TableView from './components/TableView.vue';
import ProfileForm from './components/ProfileForm.vue';
import HistoryView from './components/HistoryView.vue';
import SyncProgressModal from './components/SyncProgressModal.vue';
import MiniFloatingSyncWidget from './components/MiniFloatingSyncWidget.vue';

import {
  getStoredProfiles,
  saveProfiles,
  getActiveProfileId,
  setActiveProfileId,
} from './services/profileStore.js';
import {
  getSyncHistory,
  addHistoryEntry,
  clearSyncHistory,
} from './services/historyStore.js';
import {
  getAllTableStates,
  saveTableState,
} from './services/syncStateStore.js';
import { safeInvoke, safeListen } from './services/tauriHelper.js';

// State
const profiles = ref([]);
const activeProfileId = ref('');
const currentView = ref('tables'); // 'tables' | 'new-profile' | 'edit-profile' | 'history'

const tables = ref([]);
const tableStatesMap = ref({});
const isLoadingTables = ref(false);

const isSyncing = ref(false);
const showSyncModal = ref(false);
const syncStartedAt = ref(0);
const syncLogs = ref([]);
const syncResult = ref(null);
const isCancelling = ref(false);
const syncProgress = ref({
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
});
const historyList = ref([]);

const activeProfile = computed(() => {
  return profiles.value.find((p) => p.id === activeProfileId.value) || profiles.value[0];
});

function refreshLocalTableStates() {
  if (!activeProfile.value) return;
  tableStatesMap.value = getAllTableStates(activeProfile.value.id);
}

async function detectAndSyncLocalIds(tableList = null) {
  const profile = activeProfile.value;
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
      // Read the latest persisted values so local-ID detection cannot overwrite
      // a newer lastSyncTime with a stale value from the Vue state cache.
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
      if (activeProfileId.value === profileId) {
        refreshLocalTableStates();
      }
    }
  } catch (err) {
    console.warn('Gagal mendeteksi last local IDs:', err);
  }
}

function handleSelectProfile(id) {
  activeProfileId.value = id;
  setActiveProfileId(id);
  currentView.value = 'tables';
  refreshLocalTableStates();
  fetchTables();
}

function handleCreateProfile() {
  currentView.value = 'new-profile';
}

function handleEditProfile(id) {
  if (id) {
    activeProfileId.value = id;
    setActiveProfileId(id);
  }
  currentView.value = 'edit-profile';
}

function handleSaveProfile(savedProfile) {
  const idx = profiles.value.findIndex((p) => p.id === savedProfile.id);
  if (idx !== -1) {
    profiles.value[idx] = savedProfile;
  } else {
    profiles.value.push(savedProfile);
  }
  activeProfileId.value = savedProfile.id;
  setActiveProfileId(savedProfile.id);
  saveProfiles(profiles.value);
  currentView.value = 'tables';
  fetchTables();
}

function handleDeleteProfile(id) {
  profiles.value = profiles.value.filter((p) => p.id !== id);
  if (profiles.value.length === 0) {
    activeProfileId.value = '';
    setActiveProfileId('');
    handleCreateProfile();
  } else {
    activeProfileId.value = profiles.value[0].id;
    setActiveProfileId(activeProfileId.value);
    currentView.value = 'tables';
    fetchTables();
  }
  saveProfiles(profiles.value);
}

function handleSaveTemplate({ profileId, name, tableNames }) {
  const profile = profiles.value.find((item) => item.id === profileId);
  if (!profile) return;
  if (!profile.templates) {
    profile.templates = [];
  }
  const newTpl = {
    id: `tpl-${Date.now()}`,
    name,
    tableNames,
  };
  profile.templates.push(newTpl);
  saveProfiles(profiles.value);
}

function handleUpdateTemplate({ profileId, templateId, tableNames }) {
  const profile = profiles.value.find((item) => item.id === profileId);
  const template = profile?.templates?.find((item) => item.id === templateId);
  if (!template) return;
  template.tableNames = tableNames;
  saveProfiles(profiles.value);
}

function handleClearHistory() {
  historyList.value = clearSyncHistory();
}

// Fetch remote tables via SSH
async function fetchTables() {
  if (!activeProfile.value) return;
  refreshLocalTableStates();
  isLoadingTables.value = true;
  try {
    const list = await safeInvoke('get_remote_tables', {
      sshConfig: activeProfile.value.ssh,
      remoteDbConfig: activeProfile.value.remote_db,
    });
    tables.value = Array.isArray(list) ? list : [];
    // Also detect local primary keys and max IDs
    detectAndSyncLocalIds();
  } catch (err) {
    console.error('Gagal mengambil daftar tabel remote:', err);
    // Add log or notification if needed
  } finally {
    isLoadingTables.value = false;
  }
}

// Listeners cleanup array
let unlisteners = [];

async function setupEventListeners() {
  try {
    const unLog = await safeListen('sync-log', (event) => {
      const payload = event.payload;
      if (payload) {
        appendSyncLog(payload);
      }
    });
    unlisteners.push(unLog);

    const unProgress = await safeListen('sync-progress', (event) => {
      const payload = event.payload;
      if (payload) {
        syncProgress.value = payload;
      }
    });
    unlisteners.push(unProgress);

  } catch (e) {
    console.warn('Failed to register listeners:', e);
  }
}

async function handleStartSync(syncOptions) {
  if (isSyncing.value || !activeProfile.value) return;

  isSyncing.value = true;
  showSyncModal.value = true;
  syncStartedAt.value = Date.now();
  syncLogs.value = [];
  syncResult.value = null;
  const syncingProfile = activeProfile.value;
  const selectedTables = syncOptions.tables.map((table) => table.name);
  syncProgress.value = {
    current_table_index: 0,
    total_tables: syncOptions.tables.length,
    completed_tables: 0,
    completed_table_names: [],
    skipped_table_names: [],
    current_table_name: syncOptions.tables[0]?.name || '',
    rows_synced_current_table: 0,
    total_synced_all_tables: 0,
    processed_tables: 0,
    successful_tables: 0,
    failed_tables: 0,
    skipped_tables: 0,
    elapsed_ms: 0,
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
      const durationSec = Math.round((res.duration_ms || 0) / 1000);
      const mins = Math.floor(durationSec / 60);
      const secs = durationSec % 60;
      const durText = mins > 0 ? `${mins}m ${secs}s` : `${secs}s`;

      const failedTableNames = [
        ...new Set(
          (res.failures || [])
            .map((f) => f.table_name)
            .filter(Boolean)
        ),
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

      const newEntry = {
        profile_name: syncingProfile.name || 'database',
        profile_id: syncingProfile.id,
        status: historyStatus(status),
        tables_count: res.total_tables || 0,
        rows_copied: res.total_rows_synced || 0,
        duration_text: durText,
        // Hemat memori: hanya catat tabel gagal dan tabel dengan perubahan struktur
        failed_tables: failedTableNames,
        structure_changed_tables: structureChangedTables,
        failures: res.failures || [],
        mode: syncOptions.sync_mode || 'incremental',
        row_limit: syncOptions.row_limit,
        // Hemat memori: abaikan log tipe info yang menuhin log
        logs: syncLogs.value.filter((l) => l.message && l.message.trim() && l.type !== 'info'),
      };
      historyList.value = addHistoryEntry(newEntry);

      if ('Notification' in window && Notification.permission === 'granted') {
        const notifBody = status === 'success'
          ? `Sinkronisasi '${syncingProfile.name}' selesai (${res.total_tables || 0} tabel, ${res.total_rows_synced || 0} baris).`
          : `Sinkronisasi '${syncingProfile.name}' ${status} (${res.total_rows_synced || 0} baris tersalin).`;
        new Notification('DB-Sync-Ssh', { body: notifBody });
      }

      if (status === 'success' || status === 'partial') {
        const failedTableNames = new Set(
          (res.failures || [])
            .map((f) => f.table_name)
            .filter(Boolean)
        );
        const successfulTables = selectedTables.filter(
          (t) => !failedTableNames.has(t)
        );

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
          if (activeProfileId.value === syncingProfile.id) {
            await detectAndSyncLocalIds(successfulTables);
          }
        }
      }
    }
  } catch (err) {
    console.error('Sinkronisasi error:', err);
    syncResult.value = {
      status: 'failed',
      success: false,
      error: String(err),
      duration_ms: Date.now() - syncStartedAt.value,
    };
    syncProgress.value = {
      ...syncProgress.value,
      elapsed_ms: Date.now() - syncStartedAt.value,
      status: 'failed',
    };
    appendSyncLog({
      type: 'error',
      message: `Error: ${err}`,
      timestamp: new Date().toLocaleTimeString(),
      details: String(err),
    });
    historyList.value = addHistoryEntry({
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

    if ('Notification' in window && Notification.permission === 'granted') {
      new Notification('DB-Sync-Ssh', { body: `Sinkronisasi '${syncingProfile.name}' gagal: ${err}` });
    }
  } finally {
    isSyncing.value = false;
    isCancelling.value = false;
  }
}

function extractTableName(msg) {
  if (!msg) return null;
  const match = msg.match(/\[Tabel ['"]?([^'"\]]+)['"]?\]/);
  return match ? match[1] : null;
}

async function handleRerunSync(historyItem) {
  if (isSyncing.value) return;

  if (historyItem.profile_id && profiles.value.some((p) => p.id === historyItem.profile_id)) {
    activeProfileId.value = historyItem.profile_id;
    setActiveProfileId(historyItem.profile_id);
    refreshLocalTableStates();
    await fetchTables();
  }
  currentView.value = 'tables';

  // Prioritas tabel untuk diulang: tabel gagal & tabel perubahan struktur, atau fallback table_names riwayat lama
  const failedList = historyItem.failed_tables || (historyItem.failures || []).map((f) => f.table_name).filter(Boolean);
  const changedList = historyItem.structure_changed_tables || [];
  const targetNames = [...new Set([...failedList, ...changedList])];
  const namesToSync = targetNames.length > 0 ? targetNames : (historyItem.table_names || []);

  const tablesToSync = namesToSync.map((name) => ({
    name,
    sync_structure: true,
    sync_data: true,
  }));

  if (tablesToSync.length === 0) return;

  const watermarks = {};
  if (historyItem.mode !== 'fresh') {
    const states = getAllTableStates(activeProfileId.value);
    for (const t of tablesToSync) {
      const st = states[t.name];
      if (st?.lastSyncedId) {
        watermarks[t.name] = {
          primary_key: st.primaryKey || 'id',
          last_id: String(st.lastSyncedId).trim(),
          last_sync_time: st.lastSyncTime || null,
        };
      }
    }
  }

  handleStartSync({
    tables: tablesToSync,
    row_limit: historyItem.row_limit || null,
    sync_mode: historyItem.mode || 'incremental',
    incremental_watermarks: historyItem.mode !== 'fresh' ? watermarks : null,
    concurrency: 2,
  });
}

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

function historyStatus(status) {
  if (status === 'success') return 'Completed';
  if (status === 'partial') return 'Partial';
  if (status === 'cancelled') return 'Cancelled';
  return 'Failed';
}

async function handleStopSync() {
  if (isCancelling.value) return;
  isCancelling.value = true;
  try {
    await safeInvoke('cancel_ssh_sync');
  } catch (err) {
    console.warn('Gagal membatalkan sinkronisasi:', err);
    isCancelling.value = false;
  }
}

onMounted(async () => {
  profiles.value = getStoredProfiles();
  const storedActiveProfileId = getActiveProfileId();
  activeProfileId.value = profiles.value.some((profile) => profile.id === storedActiveProfileId)
    ? storedActiveProfileId
    : profiles.value[0]?.id || '';
  if (activeProfileId.value !== storedActiveProfileId) {
    setActiveProfileId(activeProfileId.value);
  }
  currentView.value = profiles.value.length === 0 ? 'new-profile' : 'tables';
  historyList.value = getSyncHistory();
  refreshLocalTableStates();

  await setupEventListeners();
  fetchTables();
});

onBeforeUnmount(() => {
  unlisteners.forEach((fn) => typeof fn === 'function' && fn());
});
</script>
