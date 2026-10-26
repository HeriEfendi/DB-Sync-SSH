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
            <button
              v-if="searchQuery"
              type="button"
              class="search-clear-btn"
              title="Clear search (Esc)"
              @click="clearSearch"
            >
              ✕
            </button>
          </div>

          <div class="selection-actions">
            <button class="btn-chip" title="Pilih semua tabel hasil filter (Ctrl+A)" @click="handleSelectAll">Select All</button>
            <button class="btn-chip" title="Clear all selection" @click="handleClearAll">Clear</button>
          </div>

          <div class="filter-chips-group">
            <button
              class="filter-chip"
              :class="{ active: smartFilter === 'all' }"
              @click="smartFilter = 'all'"
            >
              All
            </button>
            <button
              class="filter-chip"
              :class="{ active: smartFilter === 'selected' }"
              @click="smartFilter = 'selected'"
            >
              Selected ({{ selectedCount }})
            </button>
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

          <div class="row-limit-wrapper">
            <span class="control-label">Workers:</span>
            <select
              v-model.number="concurrencyWorkers"
              class="limit-select"
              title="Jumlah worker download SSH paralel (1-4)"
            >
              <option :value="1">1 Worker</option>
              <option :value="2">2 Workers</option>
              <option :value="3">3 Workers</option>
              <option :value="4">4 Workers</option>
            </select>
          </div>

          <div class="row-limit-wrapper">
            <span class="control-label">Auto:</span>
            <select
              v-model.number="autoSyncIntervalMinutes"
              class="limit-select"
              title="Sinkronisasi otomatis di latar belakang"
              @change="handleAutoSyncChange"
            >
              <option :value="0">Off (Manual)</option>
              <option :value="5">Tiap 5 Menit</option>
              <option :value="15">Tiap 15 Menit</option>
              <option :value="30">Tiap 30 Menit</option>
              <option :value="60">Tiap 1 Jam</option>
              <option :value="120">Tiap 2 Jam</option>
            </select>
            <span
              v-if="autoSyncIntervalMinutes > 0"
              class="auto-sync-countdown-badge"
              :title="`Sync otomatis berjalan setiap ${autoSyncIntervalMinutes} menit`"
            >
              ⏱️ {{ autoSyncCountdownText }}
            </span>
          </div>
        </div>

        <div class="toolbar-right">
          <button
            type="button"
            class="btn-secondary"
            :disabled="isLoadingTables || isSyncing || isComparingDiff"
            title="Bandingkan baris tabel remote vs lokal sebelum sync"
            @click="openDiffInspector"
          >
            <GitCompare :size="13" :class="{ 'spin-anim': isComparingDiff }" />
            <span>Compare Diff</span>
          </button>

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
            :title="selectedCount > 0 ? 'Mulai sinkronisasi tabel terpilih (Shortcut: Ctrl+Enter)' : 'Pilih minimal 1 tabel untuk sync'"
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
      <div
        ref="tableContainerRef"
        class="table-scroll-container"
        @scroll.passive="handleScroll"
      >
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
              <th class="col-action">Action</th>
            </tr>
          </thead>
          <tbody>
            <!-- Top virtual scroll spacer row -->
            <tr
              v-if="topSpacerHeight > 0"
              class="virtual-spacer-row"
              :style="{ height: `${topSpacerHeight}px` }"
              aria-hidden="true"
            >
              <td colspan="6" style="padding: 0; border: none; height: inherit; pointer-events: none; background: transparent;"></td>
            </tr>

            <tr
              v-for="tbl in visibleTables"
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
              <td class="col-action">
                <button
                  type="button"
                  class="btn-quick-sync"
                  :title="`Sync instan tabel '${tbl.name}' sekarang`"
                  :disabled="isSyncing"
                  @click.stop="quickSyncTable(tbl.name)"
                >
                  <Zap :size="11" />
                  <span>Sync</span>
                </button>
              </td>
            </tr>

            <!-- Bottom virtual scroll spacer row -->
            <tr
              v-if="bottomSpacerHeight > 0"
              class="virtual-spacer-row"
              :style="{ height: `${bottomSpacerHeight}px` }"
              aria-hidden="true"
            >
              <td colspan="6" style="padding: 0; border: none; height: inherit; pointer-events: none; background: transparent;"></td>
            </tr>

            <tr v-if="filteredTables.length === 0">
              <td colspan="6" class="empty-state-cell">
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
import { ref, computed, watch, onMounted, onBeforeUnmount } from 'vue';
import { Database, Search, RotateCw, Play, Settings, Zap, GitCompare } from 'lucide-vue-next';
import TableDiffModal from './TableDiffModal.vue';
import { safeInvoke } from '../services/tauriHelper.js';
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

const smartFilter = ref('all'); // 'all' | 'selected'
const concurrencyWorkers = ref(2);
const autoSyncIntervalMinutes = ref(0);
const autoSyncRemainingSeconds = ref(0);
let autoSyncIntervalTimer = null;

const showDiffModal = ref(false);
const isComparingDiff = ref(false);
const diffResults = ref([]);

watch(
  () => props.profile.id,
  () => {
    selectedTemplateId.value = getValidTemplateId(props.profile);
    shouldRestoreTemplate = selectedTemplateId.value !== 'all';
    const rowLimit = getLastSelectedRowLimit(props.profile.id);
    rowLimitPreset.value = rowLimit.preset;
    customRowLimit.value = rowLimit.custom;

    const storedAuto = parseInt(localStorage.getItem(`db_sync_auto_${props.profile.id}`), 10);
    autoSyncIntervalMinutes.value = !isNaN(storedAuto) && storedAuto > 0 ? storedAuto : 0;
    if (autoSyncIntervalMinutes.value > 0) {
      startAutoSyncTimer();
    } else {
      stopAutoSyncTimer();
    }
  },
  { immediate: true }
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
  let list = props.tables;

  // 1. Filter: All vs Selected Only
  if (smartFilter.value === 'selected') {
    list = list.filter((tbl) => tableStates.value[tbl.name]?.selected);
  }

  // 2. Search query (supports !negation and regex)
  const q = searchQuery.value.trim();
  if (!q) return list;

  if (q.startsWith('!')) {
    const negTerm = q.substring(1).trim().toLowerCase();
    if (negTerm) {
      return list.filter((tbl) => !tbl.name.toLowerCase().includes(negTerm));
    }
  }

  try {
    const reg = new RegExp(q, 'i');
    return list.filter((tbl) => reg.test(tbl.name));
  } catch {
    const low = q.toLowerCase();
    return list.filter((tbl) => tbl.name.toLowerCase().includes(low));
  }
});

// --- High-Performance Virtual Scrolling for 100 to 10,000+ Tables ---
const tableContainerRef = ref(null);
const scrollTop = ref(0);
const containerHeight = ref(500);
const ROW_HEIGHT = 32;
const BUFFER_ITEMS = 15;

const isVirtualScrollActive = computed(() => filteredTables.value.length > 50);

const startIndex = computed(() => {
  if (!isVirtualScrollActive.value) return 0;
  const index = Math.floor(scrollTop.value / ROW_HEIGHT) - BUFFER_ITEMS;
  return Math.max(0, index);
});

const endIndex = computed(() => {
  const total = filteredTables.value.length;
  if (!isVirtualScrollActive.value) return total;
  const count = Math.ceil(containerHeight.value / ROW_HEIGHT) + 2 * BUFFER_ITEMS;
  return Math.min(total, startIndex.value + count);
});

const visibleTables = computed(() => {
  if (!isVirtualScrollActive.value) return filteredTables.value;
  return filteredTables.value.slice(startIndex.value, endIndex.value);
});

const topSpacerHeight = computed(() => {
  if (!isVirtualScrollActive.value) return 0;
  return startIndex.value * ROW_HEIGHT;
});

const bottomSpacerHeight = computed(() => {
  if (!isVirtualScrollActive.value) return 0;
  const total = filteredTables.value.length;
  return Math.max(0, (total - endIndex.value) * ROW_HEIGHT);
});

let scrollRafId = null;
function handleScroll(e) {
  if (scrollRafId) cancelAnimationFrame(scrollRafId);
  scrollRafId = requestAnimationFrame(() => {
    if (e.target) {
      scrollTop.value = e.target.scrollTop;
    }
  });
}

function updateContainerHeight() {
  if (tableContainerRef.value) {
    containerHeight.value = tableContainerRef.value.clientHeight || 500;
  }
}

// Reset scroll to top when search query or smart filter changes
watch([searchQuery, smartFilter], () => {
  if (tableContainerRef.value) {
    tableContainerRef.value.scrollTop = 0;
  }
  scrollTop.value = 0;
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
    concurrency: concurrencyWorkers.value,
  });
}

function clearSearch() {
  searchQuery.value = '';
  searchInputRef.value?.focus();
}

function quickSyncTable(tableName) {
  if (props.isSyncing) return;

  const s = tableStates.value[tableName] || {};
  const selectedItems = [
    {
      name: tableName,
      sync_structure: true,
      sync_data: s.sync_data !== undefined ? Boolean(s.sync_data) : true,
    },
  ];

  const watermarks = {};
  const st = props.tableStatesMap[tableName];
  const lastId =
    st?.lastSyncedId === undefined || st?.lastSyncedId === null
      ? ''
      : String(st.lastSyncedId).trim();
  if (/^[0-9]+$/.test(lastId) && /[1-9]/.test(lastId)) {
    watermarks[tableName] = {
      primary_key: st.primaryKey || 'id',
      last_id: lastId,
      last_sync_time: st.lastSyncTime || null,
    };
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
    concurrency: 1,
  });
}

// Diff Inspector methods
async function openDiffInspector() {
  if (isComparingDiff.value || props.isSyncing) return;
  showDiffModal.value = true;
  isComparingDiff.value = true;
  diffResults.value = [];
  try {
    const res = await safeInvoke('compare_table_counts', {
      sshConfig: props.profile.ssh,
      remoteDbConfig: props.profile.remote_db,
      localDbConfig: props.profile.local_db,
      tables: props.tables.map((t) => t.name),
    });
    diffResults.value = Array.isArray(res) ? res : [];
  } catch (err) {
    console.error('Failed to compare table counts:', err);
  } finally {
    isComparingDiff.value = false;
  }
}

function handleSelectDiverged(divergedNames) {
  const nameSet = new Set(divergedNames);
  for (const tbl of props.tables) {
    if (tableStates.value[tbl.name]) {
      tableStates.value[tbl.name].selected = nameSet.has(tbl.name);
    }
  }
}

function handleSyncDiverged(divergedNames) {
  handleSelectDiverged(divergedNames);
  startSync();
}

// Auto-Sync methods
const autoSyncCountdownText = computed(() => {
  const m = Math.floor(autoSyncRemainingSeconds.value / 60);
  const s = autoSyncRemainingSeconds.value % 60;
  return `${m}:${String(s).padStart(2, '0')}`;
});

function handleAutoSyncChange() {
  localStorage.setItem(`db_sync_auto_${props.profile.id}`, String(autoSyncIntervalMinutes.value));
  if (autoSyncIntervalMinutes.value > 0) {
    if ('Notification' in window && Notification.permission === 'default') {
      Notification.requestPermission();
    }
    startAutoSyncTimer();
  } else {
    stopAutoSyncTimer();
  }
}

function startAutoSyncTimer() {
  stopAutoSyncTimer();
  if (autoSyncIntervalMinutes.value <= 0) return;
  autoSyncRemainingSeconds.value = autoSyncIntervalMinutes.value * 60;
  autoSyncIntervalTimer = setInterval(() => {
    if (props.isSyncing) return; // Pause countdown while syncing
    if (autoSyncRemainingSeconds.value > 0) {
      autoSyncRemainingSeconds.value -= 1;
    }
    if (autoSyncRemainingSeconds.value <= 0) {
      autoSyncRemainingSeconds.value = autoSyncIntervalMinutes.value * 60;
      startSync();
    }
  }, 1000);
}

function stopAutoSyncTimer() {
  if (autoSyncIntervalTimer) {
    clearInterval(autoSyncIntervalTimer);
    autoSyncIntervalTimer = null;
  }
}

function onKeyDown(e) {
  const isInputFocused = ['INPUT', 'TEXTAREA'].includes(document.activeElement?.tagName);

  // 1. Ctrl+F / Cmd+F: Focus search input
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'f') {
    e.preventDefault();
    if (searchInputRef.value) {
      searchInputRef.value.focus();
      searchInputRef.value.select();
    }
    return;
  }

  // 2. Ctrl+A / Cmd+A: Select all filtered tables (only when not editing an input)
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'a' && !isInputFocused) {
    e.preventDefault();
    handleSelectAll();
    return;
  }

  // 3. Ctrl+Enter / Cmd+Enter: Trigger Start Sync
  if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') {
    if (selectedCount.value > 0 && !props.isSyncing) {
      e.preventDefault();
      startSync();
    }
    return;
  }

  // 4. Escape: Close Diff modal or clear/blur search
  if (e.key === 'Escape') {
    if (showDiffModal.value) {
      showDiffModal.value = false;
      return;
    }
    if (document.activeElement === searchInputRef.value) {
      if (searchQuery.value) {
        searchQuery.value = '';
      } else {
        searchInputRef.value.blur();
      }
    }
  }
}

let resizeObserver = null;

onMounted(() => {
  window.addEventListener('keydown', onKeyDown);
  updateContainerHeight();
  if (typeof ResizeObserver !== 'undefined' && tableContainerRef.value) {
    resizeObserver = new ResizeObserver(() => {
      updateContainerHeight();
    });
    resizeObserver.observe(tableContainerRef.value);
  }
});

onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKeyDown);
  stopAutoSyncTimer();
  if (scrollRafId) cancelAnimationFrame(scrollRafId);
  if (resizeObserver) {
    resizeObserver.disconnect();
    resizeObserver = null;
  }
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

.filter-chips-group {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  margin-left: 0.375rem;
}

.filter-chip {
  background: transparent;
  border: 1px solid transparent;
  color: #64748b;
  font-size: 0.6875rem;
  font-weight: 500;
  padding: 2px 7px;
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition: all 0.15s ease;
}

.filter-chip:hover {
  background-color: #e2e8f0;
  color: #0f172a;
}

.filter-chip.active {
  background-color: var(--primary-green-light);
  border-color: rgba(var(--primary-green-rgb), 0.3);
  color: var(--primary-green);
  font-weight: 600;
}

.auto-sync-countdown-badge {
  display: inline-flex;
  align-items: center;
  background-color: var(--primary-green-light);
  color: var(--primary-green);
  border: 1px solid rgba(var(--primary-green-rgb), 0.2);
  border-radius: var(--radius-sm);
  padding: 2px 6px;
  font-size: 0.65625rem;
  font-family: var(--font-mono);
  font-weight: 600;
  white-space: nowrap;
}

.virtual-spacer-row {
  border: none !important;
  background: transparent !important;
  pointer-events: none;
}

.virtual-spacer-row td {
  padding: 0 !important;
  border: none !important;
  line-height: 0 !important;
  background: transparent !important;
  pointer-events: none;
}

:deep(.data-table tr.virtual-spacer-row:hover) {
  background-color: transparent !important;
}
</style>
