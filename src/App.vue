<template>
  <div class="app-container">
    <!-- Left Navigation Sidebar -->
    <Sidebar
      :profiles="profiles"
      :active-profile-id="activeProfileId"
      :current-view="currentView"
      @select-profile="onSelectProfile"
      @create-profile="handleCreateProfile"
      @edit-profile="handleEditProfile"
      @delete-profile="onDeleteProfile"
      @open-history="currentView = 'history'"
    />

    <!-- Main Panel Content -->
    <main class="main-panel">
      <!-- 1. Table View -->
      <TableView
        v-if="currentView === 'tables' && activeProfile"
        :profile="activeProfile"
        :tables="tables"
        :table-states-map="tableStatesMap"
        :is-loading-tables="isLoadingTables"
        :is-syncing="isSyncing"
        @refresh-tables="fetchTables(activeProfile)"
        @start-sync="onStartSync"
        @save-template="handleSaveTemplate"
        @update-template="handleUpdateTemplate"
        @edit-profile="handleEditProfile"
      />

      <!-- 2. Profile Form View -->
      <ProfileForm
        v-else-if="currentView === 'new-profile' || currentView === 'edit-profile'"
        :initial-profile="currentView === 'edit-profile' ? activeProfile : null"
        :is-editing="currentView === 'edit-profile'"
        @save="onSaveProfile"
        @cancel="currentView = 'tables'"
        @delete="onDeleteProfile"
      />

      <!-- 3. History View -->
      <HistoryView
        v-else-if="currentView === 'history'"
        :history-list="historyList"
        @clear-history="handleClearHistory"
        @rerun-sync="onRerunSync"
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
      @stop-sync="stopSync"
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
      @stop-sync="stopSync"
      @dismiss="syncResult = null"
    />
  </div>
</template>

<script setup>
import { onMounted, onBeforeUnmount } from 'vue';
import Sidebar from './components/Sidebar.vue';
import TableView from './components/TableView.vue';
import ProfileForm from './components/ProfileForm.vue';
import HistoryView from './components/HistoryView.vue';
import SyncProgressModal from './components/SyncProgressModal.vue';
import MiniFloatingSyncWidget from './components/MiniFloatingSyncWidget.vue';

import { useProfiles } from './composables/useProfiles.js';
import { useTables } from './composables/useTables.js';
import { useSync } from './composables/useSync.js';
import { useHistory } from './composables/useHistory.js';
import { getAllTableStates } from './services/syncStateStore.js';

// Initialize composables (SoC & SRP)
const {
  profiles,
  activeProfileId,
  activeProfile,
  currentView,
  initProfiles,
  handleSelectProfile,
  handleCreateProfile,
  handleEditProfile,
  handleSaveProfile,
  handleDeleteProfile,
  handleSaveTemplate,
  handleUpdateTemplate,
} = useProfiles();

const {
  tables,
  tableStatesMap,
  isLoadingTables,
  refreshLocalTableStates,
  detectAndSyncLocalIds,
  fetchTables,
} = useTables();

const {
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
} = useSync();

const {
  historyList,
  initHistory,
  handleClearHistory,
  recordHistory,
} = useHistory();

function onSelectProfile(id) {
  handleSelectProfile(id);
  refreshLocalTableStates(id);
  fetchTables(activeProfile.value);
}

function onSaveProfile(savedProfile) {
  handleSaveProfile(savedProfile);
  refreshLocalTableStates(savedProfile.id);
  fetchTables(activeProfile.value);
}

function onDeleteProfile(id) {
  handleDeleteProfile(id);
  if (activeProfile.value) {
    refreshLocalTableStates(activeProfile.value.id);
    fetchTables(activeProfile.value);
  }
}

function onStartSync(syncOptions) {
  startSync(syncOptions, activeProfile.value, {
    onHistoryRecord: recordHistory,
    onAfterSync: async (successfulTables) => {
      if (activeProfile.value) {
        await detectAndSyncLocalIds(activeProfile.value, successfulTables);
      }
    },
  });
}

async function onRerunSync(historyItem) {
  if (isSyncing.value) return;

  if (historyItem.profile_id && profiles.value.some((p) => p.id === historyItem.profile_id)) {
    handleSelectProfile(historyItem.profile_id);
    refreshLocalTableStates(historyItem.profile_id);
    await fetchTables(activeProfile.value);
  }
  currentView.value = 'tables';

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
  if (historyItem.mode !== 'fresh' && activeProfile.value) {
    const states = getAllTableStates(activeProfile.value.id);
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

  onStartSync({
    tables: tablesToSync,
    row_limit: historyItem.row_limit || null,
    sync_mode: historyItem.mode || 'incremental',
    incremental_watermarks: historyItem.mode !== 'fresh' ? watermarks : null,
    concurrency: 2,
  });
}

onMounted(async () => {
  initProfiles();
  initHistory();
  if (activeProfile.value) {
    refreshLocalTableStates(activeProfile.value.id);
    fetchTables(activeProfile.value);
  }
  await setupEventListeners();
});

onBeforeUnmount(() => {
  cleanupEventListeners();
});
</script>

<style scoped>
/* Modular styles imported in src/styles/main.css */
</style>
