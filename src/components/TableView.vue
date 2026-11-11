<template>
  <div class="table-view-container">
    <!-- Header -->
    <header class="main-header">
      <div class="header-title-group">
        <h1 class="main-title">{{ profile.name }}</h1>
        <div class="main-subtitle">
          <Database :size="13" />
          <span>mysql • {{ profile.ssh?.host || 'remote' }} → {{ profile.local_db?.host || 'localhost' }}:{{ profile.local_db?.port || 3306 }}</span>
        </div>
      </div>
      <div class="header-actions">
        <button
          type="button"
          class="btn-icon"
          title="Edit Connection Profile"
          @click="$emit('edit-profile')"
        >
          <Settings :size="15" />
        </button>
      </div>
    </header>

    <!-- Toolbar Component -->
    <TableToolbar
      :search-query="searchQuery"
      :smart-filter="smartFilter"
      :selected-count="selectedCount"
      :profile="profile"
      :selected-template-id="selectedTemplateId"
      :sync-mode="syncMode"
      :row-limit-preset="rowLimitPreset"
      :custom-row-limit="customRowLimit"
      :concurrency-workers="concurrencyWorkers"
      :auto-sync-interval-minutes="autoSyncIntervalMinutes"
      :auto-sync-countdown-text="autoSyncCountdownText"
      :is-loading-tables="isLoadingTables"
      :is-syncing="isSyncing"
      :is-comparing-diff="isComparingDiff"
      @update:search-query="searchQuery = $event"
      @update:smart-filter="smartFilter = $event"
      @update:selected-template-id="onTemplateChange"
      @update:sync-mode="syncMode = $event"
      @update:row-limit-preset="rowLimitPreset = $event"
      @update:custom-row-limit="customRowLimit = $event"
      @update:concurrency-workers="concurrencyWorkers = $event"
      @update:auto-sync-interval-minutes="setAutoSyncInterval($event, profile.id)"
      @select-all="handleSelectAll"
      @clear-all="handleClearAll"
      @save-template="handleSaveTemplate"
      @update-template="handleUpdateTemplate"
      @open-diff="openDiffInspector"
      @refresh="$emit('refresh-tables')"
      @start-sync="startSync"
    />

    <!-- Data Table Grid Component -->
    <TableDataGrid
      ref="gridRef"
      :filtered-tables="filteredTables"
      :visible-tables="visibleItems"
      :top-spacer-height="topSpacerHeight"
      :bottom-spacer-height="bottomSpacerHeight"
      :table-states="tableStates"
      :table-states-map="tableStatesMap"
      :is-all-selected="isAllSelected"
      :is-indeterminate="isIndeterminate"
      :is-all-data-checked="isAllDataChecked"
      :is-data-indeterminate="isDataIndeterminate"
      :is-loading-tables="isLoadingTables"
      :is-syncing="isSyncing"
      :total-tables-count="tables.length"
      :selected-count="selectedCount"
      @toggle-select-all="toggleSelectAllFiltered"
      @toggle-all-data="toggleAllDataFiltered"
      @toggle-table-select="toggleTableSelect"
      @toggle-mode="toggleMode"
      @quick-sync="quickSyncTable"
      @scroll="handleScroll"
    />

    <!-- Table Diff Modal -->
    <TableDiffModal
      v-if="showDiffModal"
      :diff-list="diffResults"
      :is-loading="isComparingDiff"
      :profile-name="profile.name"
      @close="showDiffModal = false"
      @select-diverged="handleSelectDiverged"
      @sync-diverged="handleSyncDiverged"
    />
  </div>
</template>

<script setup>
import { ref, watch, onMounted, onBeforeUnmount, toRef } from 'vue';
import { Database, Settings } from 'lucide-vue-next';
import { useTableSelection } from '../composables/useTableSelection.js';
import { useVirtualScroll } from '../composables/useVirtualScroll.js';
import { useAutoSync } from '../composables/useAutoSync.js';
import { useTableTemplates } from '../composables/useTableTemplates.js';
import { useTableDiff } from '../composables/useTableDiff.js';
import { buildTableSyncOptions, buildQuickSyncOptions } from '../utils/syncPayloadBuilder.js';
import TableToolbar from './organisms/table/TableToolbar.vue';
import TableDataGrid from './organisms/table/TableDataGrid.vue';
import TableDiffModal from './TableDiffModal.vue';

const props = defineProps({
  profile: { type: Object, required: true },
  tables: { type: Array, default: () => [] },
  tableStatesMap: { type: Object, default: () => ({}) },
  isLoadingTables: { type: Boolean, default: false },
  isSyncing: { type: Boolean, default: false },
});

const emit = defineEmits([
  'refresh-tables',
  'start-sync',
  'save-template',
  'update-template',
  'edit-profile',
]);

const tablesRef = toRef(props, 'tables');
const {
  tableStates,
  smartFilter,
  searchQuery,
  filteredTables,
  selectedCount,
  isAllSelected,
  isIndeterminate,
  isAllDataChecked,
  isDataIndeterminate,
  initTableStates,
  setTableSelected,
  toggleTableSelect,
  toggleSelectAllFiltered,
  handleSelectAll,
  handleClearAll,
  toggleMode,
  toggleAllDataFiltered,
} = useTableSelection(tablesRef);

const {
  visibleItems,
  topSpacerHeight,
  bottomSpacerHeight,
  handleScroll,
  resetScroll,
} = useVirtualScroll(filteredTables, 32, 15);

const gridRef = ref(null);
const syncMode = ref('incremental');
const concurrencyWorkers = ref(2);

const {
  selectedTemplateId,
  rowLimitPreset,
  customRowLimit,
  onTemplateChange,
  handleSaveTemplate,
  handleUpdateTemplate,
  checkAndRestoreTemplate,
} = useTableTemplates(props, emit, {
  tableStates,
  handleSelectAll,
  setTableSelected,
});

function startSync() {
  const options = buildTableSyncOptions({
    tableStates: tableStates.value,
    tableStatesMap: props.tableStatesMap,
    syncMode: syncMode.value,
    rowLimitPreset: rowLimitPreset.value,
    customRowLimit: customRowLimit.value,
    concurrencyWorkers: concurrencyWorkers.value,
  });
  emit('start-sync', options);
}

function quickSyncTable(tableName) {
  emit('start-sync', buildQuickSyncOptions(tableName, props.tableStatesMap));
}

const {
  showDiffModal,
  isComparingDiff,
  diffResults,
  openDiffInspector,
  handleSelectDiverged,
  handleSyncDiverged,
} = useTableDiff(props, {
  tableStates,
  setTableSelected,
  smartFilter,
  startSync,
});

const {
  autoSyncIntervalMinutes,
  autoSyncCountdownText,
  setAutoSyncInterval,
} = useAutoSync(() => {
  if (!props.isSyncing && selectedCount.value > 0) startSync();
});

watch(
  () => props.profile.id,
  () => {
    const storedAuto = parseInt(localStorage.getItem(`db_sync_auto_${props.profile.id}`), 10);
    setAutoSyncInterval(!isNaN(storedAuto) && storedAuto > 0 ? storedAuto : 0, props.profile.id);
  },
  { immediate: true }
);

watch([searchQuery, smartFilter], () => {
  resetScroll();
});

watch(
  () => [props.tables, props.isLoadingTables],
  () => {
    initTableStates();
    if (!props.isLoadingTables) {
      checkAndRestoreTemplate();
    }
  },
  { immediate: true }
);

function onGlobalKeyDown(e) {
  if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') {
    if (!props.isSyncing && selectedCount.value > 0) startSync();
  }
}

onMounted(() => {
  window.addEventListener('keydown', onGlobalKeyDown);
});

onBeforeUnmount(() => {
  window.removeEventListener('keydown', onGlobalKeyDown);
});
</script>

<style scoped>
/* Modular styles imported in src/styles/tables.css */
</style>
