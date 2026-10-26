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
          class="btn-icon"
          title="Edit Connection Profile"
          @click="$emit('edit-profile')"
        >
          <Settings :size="15" />
        </button>
      </div>
    </header>

    <!-- Toolbar: Desktop Card -->
    <div class="table-toolbar">
      <div class="toolbar-top">
        <div class="toolbar-left">
          <div class="search-input-wrapper">
            <Search class="search-icon" :size="13" />
            <input
              ref="searchInputRef"
              v-model="searchQuery"
              type="text"
              class="search-input"
              placeholder="Search tables... (Ctrl+F)"
            />
          </div>

          <div class="selection-actions">
            <button class="btn-chip" title="Select all filtered tables" @click="handleSelectAll">Select All</button>
            <button class="btn-chip" title="Clear all selection" @click="handleClearAll">Clear</button>
          </div>
        </div>

        <div class="toolbar-right">
          <div class="template-group">
            <span class="control-label">Template:</span>
            <select
              v-model="selectedTemplateId"
              class="template-select"
              @change="applyTemplate"
            >
              <option value="all">All Tables</option>
              <option
                v-for="tpl in profile.templates || []"
                :key="tpl.id"
                :value="tpl.id"
              >
                {{ tpl.name }}
              </option>
            </select>
            <button class="btn-secondary" @click="handleSaveTemplate">Save As New</button>
            <button
              class="btn-secondary"
              :disabled="selectedTemplateId === 'all'"
              @click="handleUpdateTemplate"
            >
              Update Template
            </button>
          </div>
        </div>
      </div>

      <div class="toolbar-divider"></div>

      <div class="toolbar-bottom">
        <div class="toolbar-left">
          <div class="button-group-wrapper">
            <span class="control-label">Mode:</span>
            <div class="pill-group">
              <button
                class="pill-btn"
                :class="{ active: syncMode === 'incremental' }"
                title="Sync Server (New & Update): Hanya menyalin baris data baru setelah Last ID dan mengupdate baris yang ada (REPLACE INTO)"
                @click="syncMode = 'incremental'"
              >
                Sync New &amp; Update
              </button>
              <button
                class="pill-btn warning-pill"
                :class="{ active: syncMode === 'fresh' }"
                title="Fresh Sync: Timpa bersih seluruh tabel lokal dari awal"
                @click="syncMode = 'fresh'"
              >
                Fresh Sync
              </button>
            </div>
          </div>

          <div class="row-limit-wrapper">
            <span class="control-label">Limit:</span>
            <select
              v-model="rowLimitPreset"
              class="limit-select"
              title="Batasan jumlah baris yang akan disalin per tabel"
            >
              <option :value="0">Unlimited (Semua)</option>
              <option :value="1000">1.000 Row</option>
              <option :value="10000">10.000 Row</option>
              <option :value="50000">50.000 Row</option>
              <option :value="100000">100.000 Row</option>
              <option :value="500000">500.000 Row</option>
              <option :value="1000000">1.000.000 Row</option>
              <option :value="2000000">2.000.000 Row</option>
              <option :value="5000000">5.000.000 Row</option>
              <option value="custom">Custom...</option>
            </select>
            <input
              v-if="rowLimitPreset === 'custom'"
              v-model.number="customRowLimit"
              type="number"
              min="1"
              class="limit-custom-input"
              placeholder="Jml baris"
            />
          </div>
        </div>

        <div class="toolbar-right">
          <button
            class="btn-refresh"
            :disabled="isLoadingTables"
            @click="$emit('refresh-tables')"
          >
            <RotateCw :size="13" :class="{ 'spin-anim': isLoadingTables }" />
            <span>Refresh</span>
          </button>

          <button
            class="btn-primary"
            :disabled="selectedCount === 0 || isSyncing"
            @click="startSync"
          >
            <Play :size="13" />
            <span>Start Sync ({{ selectedCount }})</span>
          </button>
        </div>
      </div>
    </div>

    <!-- Table Card with Fixed Header and Scrollable Body -->
    <div class="table-card">
      <div class="table-scroll-container">
        <table class="data-table">
          <thead>
            <tr>
              <th class="col-select">
                <input
                  type="checkbox"
                  :checked="isAllSelected"
                  :indeterminate.prop="isIndeterminate"
                  title="Toggle Select All"
                  @change="toggleSelectAllFiltered($event.target.checked)"
                />
              </th>
              <th>Table Name</th>
              <th class="col-last-id">Last ID</th>
              <th class="col-last-sync">Last Sync</th>
              <th class="col-sync-mode">Sync Mode</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="tbl in filteredTables"
              :key="tbl.name"
              :class="{
                'row-selected': tableStates[tbl.name]?.selected,
                'row-deselected': !tableStates[tbl.name]?.selected
              }"
            >
              <td class="col-select">
                <input
                  type="checkbox"
                  :checked="tableStates[tbl.name]?.selected"
                  @change="toggleTableSelect(tbl.name, $event.target.checked)"
                />
              </td>
              <td
                class="col-table-name"
                title="Click to toggle selection"
                @click="toggleTableSelect(tbl.name, !tableStates[tbl.name]?.selected)"
              >
                {{ tbl.name }}
              </td>
              <td class="col-last-id">
                <span
                  v-if="tableStatesMap[tbl.name]?.lastSyncedId !== undefined && tableStatesMap[tbl.name]?.lastSyncedId !== null"
                  class="last-id-pill"
                  :title="'Primary key: ' + (tableStatesMap[tbl.name].primaryKey || 'id')"
                >
                  <span class="lbl">ID:</span>
                  <strong>{{ formatLastId(tableStatesMap[tbl.name].lastSyncedId) }}</strong>
                </span>
                <span v-else class="text-dim-sync">-</span>
              </td>
              <td class="col-last-sync">
                <span
                  v-if="tableStatesMap[tbl.name]?.lastSyncTime"
                  class="last-time-text"
                  :title="formatFullDate(
                    tableStatesMap[tbl.name].lastSyncTime,
                    tableStatesMap[tbl.name].lastSyncTimeEpochMs
                  )"
                >
                  {{ formatDisplayTime(
                    tableStatesMap[tbl.name].lastSyncTime,
                    tableStatesMap[tbl.name].lastSyncTimeEpochMs
                  ) }}
                </span>
                <span v-else class="text-dim-sync">Belum sync</span>
              </td>
              <td class="col-sync-mode">
                <div class="sync-mode-checks">
                  <label class="check-label disabled" title="Struktur tabel otomatis disinkronkan di background">
                    <input
                      type="checkbox"
                      :checked="true"
                      disabled
                    />
                    <span>Structure</span>
                  </label>
                  <label class="check-label" title="Centang untuk menyalin isi data tabel">
                    <input
                      type="checkbox"
                      :checked="tableStates[tbl.name]?.sync_data"
                      @change="toggleMode(tbl.name, 'sync_data', $event.target.checked)"
                    />
                    <span>Data</span>
                  </label>
                </div>
              </td>
            </tr>
            <tr v-if="filteredTables.length === 0">
              <td colspan="5" class="empty-state-cell">
                <span v-if="isLoadingTables">Memuat daftar tabel remote...</span>
                <span v-else>Tidak ada tabel yang cocok dengan pencarian atau koneksi belum dimuat.</span>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
      <div class="table-footer-status">
        <span>Menampilkan <strong>{{ filteredTables.length }}</strong> tabel</span>
        <span>Terpilih: <strong style="color: var(--primary-green)">{{ selectedCount }}</strong> dari {{ tables.length }} tabel</span>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, watch, onMounted, onBeforeUnmount } from 'vue';
import { Database, Search, RotateCw, Play, Settings } from 'lucide-vue-next';
import {
  getLastSelectedRowLimit,
  getLastSelectedTemplateId,
  setLastSelectedRowLimit,
  setLastSelectedTemplateId,
} from '../services/profileStore.js';

const props = defineProps({
  profile: {
    type: Object,
    required: true,
  },
  tables: {
    type: Array,
    default: () => [],
  },
  tableStatesMap: {
    type: Object,
    default: () => ({}),
  },
  isLoadingTables: {
    type: Boolean,
    default: false,
  },
  isSyncing: {
    type: Boolean,
    default: false,
  },
});

const emit = defineEmits([
  'refresh-tables',
  'start-sync',
  'save-template',
  'update-template',
  'edit-profile',
]);

const searchInputRef = ref(null);
const searchQuery = ref('');
const getValidTemplateId = (profile) => {
  const storedId = getLastSelectedTemplateId(profile.id);
  if (storedId === 'all' || (profile.templates || []).some((template) => template.id === storedId)) {
    return storedId;
  }
  setLastSelectedTemplateId(profile.id, 'all');
  return 'all';
};
const selectedTemplateId = ref(getValidTemplateId(props.profile));
let shouldRestoreTemplate = selectedTemplateId.value !== 'all';
const syncMode = ref('incremental'); // 'incremental' | 'fresh'
const initialRowLimit = getLastSelectedRowLimit(props.profile.id);
const rowLimitPreset = ref(initialRowLimit.preset); // 0 = unlimited, numbers = preset, 'custom' = custom input
const customRowLimit = ref(initialRowLimit.custom);

// Per-table state: { selected: bool, sync_structure: bool, sync_data: bool }
const tableStates = ref({});

watch(
  () => props.profile.id,
  () => {
    selectedTemplateId.value = getValidTemplateId(props.profile);
    shouldRestoreTemplate = selectedTemplateId.value !== 'all';
    const rowLimit = getLastSelectedRowLimit(props.profile.id);
    rowLimitPreset.value = rowLimit.preset;
    customRowLimit.value = rowLimit.custom;
  }
);

watch(
  () => [rowLimitPreset.value, customRowLimit.value],
  ([preset, custom]) => {
    setLastSelectedRowLimit(props.profile.id, preset, custom);
  }
);

function initTableStates() {
  const newStates = {};
  props.tables.forEach((tbl) => {
    newStates[tbl.name] = {
      selected: true,
      sync_structure: true,
      sync_data: true,
    };
  });
  tableStates.value = newStates;
}

watch(
  () => [props.tables, props.isLoadingTables],
  () => {
    initTableStates();
    if (!props.isLoadingTables && shouldRestoreTemplate && props.tables.length > 0) {
      shouldRestoreTemplate = false;
      applyTemplate();
    }
  },
  { immediate: true }
);

const filteredTables = computed(() => {
  if (!searchQuery.value.trim()) {
    return props.tables;
  }
  const q = searchQuery.value.toLowerCase().trim();
  return props.tables.filter((tbl) => tbl.name.toLowerCase().includes(q));
});

const selectedCount = computed(() => {
  return Object.values(tableStates.value).filter((s) => s.selected).length;
});

const isAllSelected = computed(() => {
  if (filteredTables.value.length === 0) return false;
  return filteredTables.value.every((tbl) => tableStates.value[tbl.name]?.selected);
});

const isIndeterminate = computed(() => {
  const selectedInFiltered = filteredTables.value.filter(
    (tbl) => tableStates.value[tbl.name]?.selected
  ).length;
  return selectedInFiltered > 0 && selectedInFiltered < filteredTables.value.length;
});

function toggleSelectAllFiltered(checked) {
  filteredTables.value.forEach((tbl) => {
    setTableSelected(tbl.name, checked);
  });
}

function handleSelectAll() {
  const isFiltered = searchQuery.value.trim() !== '';
  if (isFiltered) {
    filteredTables.value.forEach((tbl) => {
      setTableSelected(tbl.name, true);
    });
  } else {
    Object.keys(tableStates.value).forEach((k) => {
      setTableSelected(k, true);
    });
  }
}

function handleClearAll() {
  const isFiltered = searchQuery.value.trim() !== '';
  if (isFiltered) {
    filteredTables.value.forEach((tbl) => {
      setTableSelected(tbl.name, false);
    });
  } else {
    Object.keys(tableStates.value).forEach((k) => {
      setTableSelected(k, false);
    });
  }
}

function setGlobalMode(mode) {
  globalMode.value = mode;
  Object.keys(tableStates.value).forEach((k) => {
    const state = tableStates.value[k];
    if (mode === 'both') {
      state.sync_structure = true;
      state.sync_data = state.selected;
    } else {
      state.sync_structure = true;
      state.sync_data = false;
    }
  });
}

function setTableSelected(tableName, selected) {
  const state = tableStates.value[tableName];
  if (state) {
    state.selected = selected;
    if (!selected) {
      state.sync_data = false;
    }
  }
}

function toggleTableSelect(tableName, checked) {
  setTableSelected(tableName, checked);
}

function toggleMode(tableName, field, checked) {
  if (tableStates.value[tableName]) {
    tableStates.value[tableName][field] = checked;
    if (checked) {
      tableStates.value[tableName].selected = true;
    }
  }
}

function applyTemplate() {
  setLastSelectedTemplateId(props.profile.id, selectedTemplateId.value);
  shouldRestoreTemplate = false;
  if (selectedTemplateId.value === 'all') {
    handleSelectAll();
    return;
  }
  const tpl = (props.profile.templates || []).find((t) => t.id === selectedTemplateId.value);
  if (tpl && Array.isArray(tpl.tableNames)) {
    const set = new Set(tpl.tableNames);
    Object.keys(tableStates.value).forEach((tblName) => {
      setTableSelected(tblName, set.has(tblName));
    });
  }
}

function handleSaveTemplate() {
  const name = window.prompt('Masukkan nama template untuk pilihan tabel saat ini:');
  if (!name || !name.trim()) return;

  const selectedNames = Object.entries(tableStates.value)
    .filter(([_, s]) => s.selected)
    .map(([tblName]) => tblName);

  emit('save-template', {
    profileId: props.profile.id,
    name: name.trim(),
    tableNames: selectedNames,
  });
}

function handleUpdateTemplate() {
  if (selectedTemplateId.value === 'all') return;
  const template = (props.profile.templates || []).find(
    (item) => item.id === selectedTemplateId.value
  );
  if (!template) return;

  const selectedNames = Object.entries(tableStates.value)
    .filter(([_, state]) => state.selected)
    .map(([tableName]) => tableName);

  emit('update-template', {
    profileId: props.profile.id,
    templateId: template.id,
    tableNames: selectedNames,
  });
}

function formatNumber(n) {
  if (n === undefined || n === null) return '0';
  return n.toString().replace(/\B(?=(\d{3})+(?!\d))/g, '.');
}

function formatLastId(id) {
  if (id === null || id === undefined || id === '') return '-';
  return String(id).replace(/\B(?=(\d{3})+(?!\d))/g, '.');
}

function getSyncDate(timeStr, epochMs) {
  if (epochMs !== undefined && epochMs !== null && Number.isFinite(Number(epochMs))) {
    const date = new Date(Number(epochMs));
    if (!isNaN(date.getTime())) return date;
  }
  if (!timeStr) return null;
  try {
    const date = new Date(timeStr.includes(' ') ? timeStr.replace(' ', 'T').slice(0, 23) : timeStr);
    return isNaN(date.getTime()) ? null : date;
  } catch {
    return null;
  }
}

function formatDisplayTime(timeStr, epochMs) {
  const date = getSyncDate(timeStr, epochMs);
  if (!date) return '';
  const parts = new Intl.DateTimeFormat('id-ID', {
    timeZone: 'Asia/Jakarta',
    day: '2-digit',
    month: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    hourCycle: 'h23',
  }).formatToParts(date);
  const part = (type) => parts.find((item) => item.type === type)?.value || '';
  return `${part('day')}/${part('month')} ${part('hour')}:${part('minute')}`;
}

function formatFullDate(timeStr, epochMs) {
  const date = getSyncDate(timeStr, epochMs);
  if (!date) return '';
  return new Intl.DateTimeFormat('id-ID', {
    timeZone: 'Asia/Jakarta',
    year: 'numeric',
    month: 'long',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
    hourCycle: 'h23',
    timeZoneName: 'short',
  }).format(date);
}

function startSync() {
  const selectedItems = [];
  const watermarks = {};

  for (const [tblName, s] of Object.entries(tableStates.value)) {
    if (s.selected) {
      selectedItems.push({
        name: tblName,
        sync_structure: true,
        sync_data: Boolean(s.sync_data),
      });

      // Pass both row ID and the remote timestamp from the previous successful sync.
      const st = props.tableStatesMap[tblName];
      const lastId = st?.lastSyncedId === undefined || st?.lastSyncedId === null
        ? ''
        : String(st.lastSyncedId).trim();
      if (/^[0-9]+$/.test(lastId) && /[1-9]/.test(lastId)) {
        watermarks[tblName] = {
          primary_key: st.primaryKey || 'id',
          last_id: lastId,
          last_sync_time: st.lastSyncTime || null,
        };
      }
    }
  }

  let rowLimit = null;
  if (rowLimitPreset.value === 'custom') {
    const customNum = parseInt(customRowLimit.value, 10);
    if (!isNaN(customNum) && customNum > 0) {
      rowLimit = customNum;
    }
  } else if (typeof rowLimitPreset.value === 'number' && rowLimitPreset.value > 0) {
    rowLimit = rowLimitPreset.value;
  }

  emit('start-sync', {
    tables: selectedItems,
    row_limit: rowLimit,
    sync_mode: syncMode.value,
    incremental_watermarks: syncMode.value === 'incremental' ? watermarks : null,
  });
}

function onKeyDown(e) {
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'f') {
    e.preventDefault();
    if (searchInputRef.value) {
      searchInputRef.value.focus();
      searchInputRef.value.select();
    }
  }
}

onMounted(() => {
  window.addEventListener('keydown', onKeyDown);
});

onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKeyDown);
});
</script>

<style scoped>
.spin-anim {
  animation: spin 1s linear infinite;
}
@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}
</style>
