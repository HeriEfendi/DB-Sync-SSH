<template>
  <div class="table-view-container">
    <!-- Header -->
    <header class="main-header">
      <div class="header-title-group">
        <h1 class="main-title">{{ profile.name }}</h1>
        <div class="main-subtitle">
          <Database :size="15" />
          <span>mysql • {{ profile.ssh?.host || 'remote' }} → {{ profile.local_db?.host || 'localhost' }}:{{ profile.local_db?.port || 3306 }}</span>
        </div>
      </div>
      <div class="header-actions">
        <button
          class="btn-secondary"
          title="Edit Connection Profile"
          @click="$emit('edit-profile')"
        >
          <Settings :size="16" />
        </button>
      </div>
    </header>

    <!-- Filter & Template Controls -->
    <div class="filter-row">
      <div class="search-input-wrapper">
        <Search class="search-icon" />
        <input
          ref="searchInputRef"
          v-model="searchQuery"
          type="text"
          class="search-input"
          placeholder="Search tables... (Ctrl+F)"
        />
      </div>

      <div class="selection-actions">
        <button class="link-btn" @click="handleSelectAll">Select All</button>
        <span style="color: #cbd5e1">|</span>
        <button class="link-btn" @click="handleClearAll">Clear</button>
      </div>

      <div class="template-group">
        <span class="template-label">Template:</span>
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
      </div>
    </div>

    <!-- Action Bar -->
    <div class="action-row">
      <div class="action-left">
        <div class="button-group-wrapper">
          <span class="group-label">Set All To:</span>
          <div class="pill-group">
            <button
              class="pill-btn"
              :class="{ active: globalMode === 'both' }"
              @click="setGlobalMode('both')"
            >
              Data + Structure
            </button>
            <button
              class="pill-btn"
              :class="{ active: globalMode === 'structure' }"
              @click="setGlobalMode('structure')"
            >
              Structure Only
            </button>
          </div>
        </div>

        <div class="row-limit-wrapper">
          <span class="group-label">Row Limit:</span>
          <input
            v-model="rowLimitInput"
            type="text"
            class="limit-input"
            placeholder="Unlimited"
          />
        </div>
      </div>

      <div class="action-right">
        <button
          class="btn-refresh"
          :disabled="isLoadingTables"
          @click="$emit('refresh-tables')"
        >
          <RotateCw :size="15" :class="{ 'spin-anim': isLoadingTables }" />
          <span>Refresh</span>
        </button>

        <button
          class="btn-primary"
          :disabled="selectedCount === 0 || isSyncing"
          @click="startSync"
        >
          <Play :size="16" />
          <span>Start Sync ({{ selectedCount }})</span>
        </button>
      </div>
    </div>

    <!-- Table List -->
    <div class="table-card">
      <table class="data-table">
        <thead>
          <tr>
            <th class="col-select">Select</th>
            <th>Table Name</th>
            <th class="col-total-rows">Total Rows</th>
            <th class="col-sync-mode">Sync Mode</th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="tbl in filteredTables"
            :key="tbl.name"
            :class="{ 'row-deselected': !tableStates[tbl.name]?.selected }"
          >
            <td class="col-select">
              <input
                type="checkbox"
                :checked="tableStates[tbl.name]?.selected"
                @change="toggleTableSelect(tbl.name, $event.target.checked)"
              />
            </td>
            <td class="col-table-name">{{ tbl.name }}</td>
            <td class="col-total-rows">{{ formatNumber(tbl.total_rows) }}</td>
            <td class="col-sync-mode">
              <div class="sync-mode-checks">
                <label class="check-label">
                  <input
                    type="checkbox"
                    :checked="tableStates[tbl.name]?.sync_structure"
                    @change="toggleMode(tbl.name, 'sync_structure', $event.target.checked)"
                  />
                  <span>Structure</span>
                </label>
                <label class="check-label">
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
            <td colspan="4" style="text-align: center; padding: 2rem; color: #64748b;">
              <span v-if="isLoadingTables">Memuat daftar tabel remote...</span>
              <span v-else>Tidak ada tabel yang cocok dengan pencarian atau koneksi belum dimuat.</span>
            </td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, watch, onMounted, onBeforeUnmount } from 'vue';
import { Database, Search, RotateCw, Play, Settings } from 'lucide-vue-next';

const props = defineProps({
  profile: {
    type: Object,
    required: true,
  },
  tables: {
    type: Array,
    default: () => [],
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

const emit = defineEmits(['refresh-tables', 'start-sync', 'save-template', 'edit-profile']);

const searchInputRef = ref(null);
const searchQuery = ref('');
const selectedTemplateId = ref('all');
const globalMode = ref('both'); // 'both' | 'structure'
const rowLimitInput = ref('Unlimited');

// Per-table state: { selected: bool, sync_structure: bool, sync_data: bool }
const tableStates = ref({});

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
  () => props.tables,
  () => {
    initTableStates();
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

function handleSelectAll() {
  const isFiltered = searchQuery.value.trim() !== '';
  if (isFiltered) {
    filteredTables.value.forEach((tbl) => {
      if (tableStates.value[tbl.name]) {
        tableStates.value[tbl.name].selected = true;
      }
    });
  } else {
    Object.keys(tableStates.value).forEach((k) => {
      tableStates.value[k].selected = true;
    });
  }
}

function handleClearAll() {
  const isFiltered = searchQuery.value.trim() !== '';
  if (isFiltered) {
    filteredTables.value.forEach((tbl) => {
      if (tableStates.value[tbl.name]) {
        tableStates.value[tbl.name].selected = false;
      }
    });
  } else {
    Object.keys(tableStates.value).forEach((k) => {
      tableStates.value[k].selected = false;
    });
  }
}

function setGlobalMode(mode) {
  globalMode.value = mode;
  Object.keys(tableStates.value).forEach((k) => {
    if (mode === 'both') {
      tableStates.value[k].sync_structure = true;
      tableStates.value[k].sync_data = true;
    } else {
      tableStates.value[k].sync_structure = true;
      tableStates.value[k].sync_data = false;
    }
  });
}

function toggleTableSelect(tableName, checked) {
  if (tableStates.value[tableName]) {
    tableStates.value[tableName].selected = checked;
  }
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
  if (selectedTemplateId.value === 'all') {
    handleSelectAll();
    return;
  }
  const tpl = (props.profile.templates || []).find((t) => t.id === selectedTemplateId.value);
  if (tpl && Array.isArray(tpl.tableNames)) {
    const set = new Set(tpl.tableNames);
    Object.keys(tableStates.value).forEach((tblName) => {
      tableStates.value[tblName].selected = set.has(tblName);
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
    name: name.trim(),
    tableNames: selectedNames,
  });
}

function formatNumber(n) {
  if (n === undefined || n === null) return '0';
  return n.toString().replace(/\B(?=(\d{3})+(?!\d))/g, '.');
}

function startSync() {
  const selectedItems = [];
  for (const [tblName, s] of Object.entries(tableStates.value)) {
    if (s.selected) {
      selectedItems.push({
        name: tblName,
        sync_structure: Boolean(s.sync_structure),
        sync_data: Boolean(s.sync_data),
      });
    }
  }

  let rowLimit = null;
  const rawLim = rowLimitInput.value.trim().toLowerCase();
  if (rawLim && rawLim !== 'unlimited') {
    const parsed = parseInt(rawLim, 10);
    if (!isNaN(parsed) && parsed > 0) {
      rowLimit = parsed;
    }
  }

  emit('start-sync', {
    tables: selectedItems,
    row_limit: rowLimit,
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
