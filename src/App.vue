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
      />
    </main>

    <!-- Live Sync Modal with Real-time Logs -->
    <SyncProgressModal
      v-if="showSyncModal"
      :profile-name="activeProfile?.name || ''"
      :progress="syncProgress"
      :logs="syncLogs"
      :is-finished="!isSyncing"
      :result="syncResult"
      :is-cancelling="isCancelling"
      @stop-sync="handleStopSync"
      @minimize="showSyncModal = false"
      @close="showSyncModal = false"
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
const syncLogs = ref([]);
const syncResult = ref(null);
const isCancelling = ref(false);
const syncProgress = ref({
  current_table_index: 0,
  total_tables: 0,
  current_table_name: '',
  rows_synced_current_table: 0,
  total_synced_all_tables: 0,
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
  if (!activeProfile.value?.local_db) return;
  try {
    const targetNames = tableList || tables.value.map((t) => t.name);
    if (!targetNames || targetNames.length === 0) return;

    const localInfos = await safeInvoke('get_all_tables_last_local_ids', {
      config: activeProfile.value.local_db,
      tables: targetNames,
    });

    if (Array.isArray(localInfos)) {
      for (const info of localInfos) {
        const existing = tableStatesMap.value[info.table_name] || {};
        const updated = {
          ...existing,
          primaryKey: info.primary_key || existing.primaryKey || 'id',
        };
        if (info.max_id !== null && info.max_id !== undefined) {
          updated.lastSyncedId = info.max_id;
        }
        saveTableState(activeProfile.value.id, info.table_name, updated);
      }
      refreshLocalTableStates();
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
    handleCreateProfile();
  } else {
    activeProfileId.value = profiles.value[0].id;
    setActiveProfileId(activeProfileId.value);
    currentView.value = 'tables';
    fetchTables();
  }
  saveProfiles(profiles.value);
}

function handleSaveTemplate({ name, tableNames }) {
  if (!activeProfile.value) return;
  if (!activeProfile.value.templates) {
    activeProfile.value.templates = [];
  }
  const newTpl = {
    id: `tpl-${Date.now()}`,
    name,
    tableNames,
  };
  activeProfile.value.templates.push(newTpl);
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
  syncLogs.value = [];
  syncResult.value = null;
  const syncingProfile = activeProfile.value;
  const selectedTables = syncOptions.tables.map((table) => table.name);
  syncProgress.value = {
    current_table_index: 0,
    total_tables: syncOptions.tables.length,
    current_table_name: syncOptions.tables[0]?.name || '',
    rows_synced_current_table: 0,
    total_synced_all_tables: 0,
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
      },
    });

    if (res) {
      const status = res.status || (res.success ? 'success' : 'failed');
      syncResult.value = { ...res, status };
      syncProgress.value = { ...syncProgress.value, status };
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

      const newEntry = {
        profile_name: syncingProfile.name || 'database',
        status: historyStatus(status),
        tables_count: res.total_tables || 0,
        rows_copied: res.total_rows_synced || 0,
        duration_text: durText,
      };
      historyList.value = addHistoryEntry(newEntry);

      if (status === 'success') {
        const nowIso = new Date().toISOString();
        for (const tableName of selectedTables) {
          saveTableState(syncingProfile.id, tableName, {
            lastSyncTime: nowIso,
          });
        }
        if (activeProfileId.value === syncingProfile.id) {
          await detectAndSyncLocalIds(selectedTables);
        }
      }
    }
  } catch (err) {
    console.error('Sinkronisasi error:', err);
    syncResult.value = {
      status: 'failed',
      success: false,
      error: String(err),
    };
    syncProgress.value = { ...syncProgress.value, status: 'failed' };
    appendSyncLog({
      type: 'error',
      message: `Error: ${err}`,
      timestamp: new Date().toLocaleTimeString(),
      details: String(err),
    });
    historyList.value = addHistoryEntry({
      profile_name: syncingProfile.name || 'database',
      status: 'Failed',
      tables_count: 0,
      rows_copied: 0,
      duration_text: '—',
    });
  } finally {
    isSyncing.value = false;
    isCancelling.value = false;
  }
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
  activeProfileId.value = getActiveProfileId();
  historyList.value = getSyncHistory();
  refreshLocalTableStates();

  await setupEventListeners();
  fetchTables();
});

onBeforeUnmount(() => {
  unlisteners.forEach((fn) => typeof fn === 'function' && fn());
});
</script>
