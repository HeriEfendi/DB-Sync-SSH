<template>
  <div class="modal-overlay" @click.self="$emit('close')">
    <div class="diff-modal-content">
      <!-- Modal Header -->
      <div class="diff-modal-header">
        <div class="header-left">
          <GitCompare :size="16" class="header-icon" />
          <div>
            <h2 class="diff-title">Diff Inspector (Perbandingan Baris Tabel)</h2>
            <p class="diff-subtitle">{{ profileName }} • Remote vs MySQL Lokal</p>
          </div>
        </div>
        <button type="button" class="btn-close" title="Tutup (Esc)" @click="$emit('close')">
          ✕
        </button>
      </div>

      <!-- Loading State -->
      <div v-if="isLoading" class="diff-loading-container">
        <RotateCw :size="24" class="spin-anim loading-icon" />
        <p class="loading-text">Menghubungkan ke remote via SSH dan membaca metadata baris tabel...</p>
      </div>

      <!-- Content State -->
      <div v-else class="diff-modal-body">
        <!-- Metric Cards -->
        <div class="metrics-grid">
          <div class="metric-card">
            <span class="metric-label">Total Tabel</span>
            <strong class="metric-val">{{ diffList.length }}</strong>
          </div>
          <div class="metric-card metric-diverged">
            <span class="metric-label">Ada Perubahan</span>
            <strong class="metric-val">{{ divergedCount }}</strong>
          </div>
          <div class="metric-card metric-new">
            <span class="metric-label">Baru di Remote</span>
            <strong class="metric-val">{{ remoteOnlyCount }}</strong>
          </div>
          <div class="metric-card metric-insync">
            <span class="metric-label">Identik / In-Sync</span>
            <strong class="metric-val">{{ inSyncCount }}</strong>
          </div>
        </div>

        <!-- Filter & Search Toolbar -->
        <div class="diff-toolbar">
          <div class="diff-tabs">
            <button
              type="button"
              class="diff-tab"
              :class="{ active: activeFilter === 'all' }"
              @click="activeFilter = 'all'"
            >
              Semua ({{ diffList.length }})
            </button>
            <button
              type="button"
              class="diff-tab tab-diverged"
              :class="{ active: activeFilter === 'diverged' }"
              @click="activeFilter = 'diverged'"
            >
              Berbeda Saja ({{ divergedCount + remoteOnlyCount }})
            </button>
            <button
              type="button"
              class="diff-tab tab-insync"
              :class="{ active: activeFilter === 'in_sync' }"
              @click="activeFilter = 'in_sync'"
            >
              Identik ({{ inSyncCount }})
            </button>
          </div>

          <div class="diff-search-box">
            <Search :size="12" class="diff-search-icon" />
            <input
              v-model="searchQuery"
              type="text"
              placeholder="Cari tabel..."
              class="diff-search-input"
            />
            <button
              v-if="searchQuery"
              type="button"
              class="diff-search-clear"
              @click="searchQuery = ''"
            >
              ✕
            </button>
          </div>
        </div>

        <!-- Data Table -->
        <div class="diff-table-container">
          <table class="diff-table">
            <thead>
              <tr>
                <th>Nama Tabel</th>
                <th style="text-align: right; width: 110px;">Remote Rows</th>
                <th style="text-align: right; width: 110px;">Lokal Rows</th>
                <th style="text-align: right; width: 100px;">Selisih</th>
                <th style="text-align: center; width: 130px;">Status</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="item in filteredList" :key="item.table_name">
                <td class="col-diff-name font-mono">{{ item.table_name }}</td>
                <td class="col-diff-num font-mono">
                  {{ item.remote_rows >= 0 ? formatNumber(item.remote_rows) : '-' }}
                </td>
                <td class="col-diff-num font-mono">
                  {{ item.local_rows >= 0 ? formatNumber(item.local_rows) : 'Belum Ada' }}
                </td>
                <td class="col-diff-diff font-mono">
                  <span
                    v-if="item.status === 'diverged'"
                    :class="item.row_diff > 0 ? 'diff-plus' : 'diff-minus'"
                  >
                    {{ item.row_diff > 0 ? '+' : '' }}{{ formatNumber(item.row_diff) }}
                  </span>
                  <span v-else-if="item.status === 'remote_only'" class="diff-plus">
                    +{{ formatNumber(item.remote_rows) }}
                  </span>
                  <span v-else-if="item.status === 'in_sync'" class="diff-zero">
                    0
                  </span>
                  <span v-else class="diff-zero">-</span>
                </td>
                <td style="text-align: center;">
                  <span
                    class="diff-badge"
                    :class="{
                      'badge-diverged': item.status === 'diverged',
                      'badge-insync': item.status === 'in_sync',
                      'badge-new': item.status === 'remote_only',
                      'badge-local': item.status === 'local_only',
                    }"
                  >
                    <span v-if="item.status === 'diverged'">⚡ Beda</span>
                    <span v-else-if="item.status === 'in_sync'">✓ Identik</span>
                    <span v-else-if="item.status === 'remote_only'">🆕 Remote Baru</span>
                    <span v-else>{{ item.status }}</span>
                  </span>
                </td>
              </tr>
              <tr v-if="filteredList.length === 0">
                <td colspan="5" class="diff-empty-cell">
                  Tidak ada tabel yang cocok dengan filter.
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>

      <!-- Modal Footer -->
      <div class="diff-modal-footer">
        <div class="footer-left">
          <span v-if="divergedCount + remoteOnlyCount > 0" class="diverged-hint">
            💡 Ditemukan <strong>{{ divergedCount + remoteOnlyCount }}</strong> tabel yang berbeda atau baru.
          </span>
          <span v-else class="diverged-hint in-sync-hint">
            🎉 Semua tabel saat ini dalam kondisi sinkron identik!
          </span>
        </div>

        <div class="footer-right">
          <button
            v-if="divergedCount + remoteOnlyCount > 0"
            type="button"
            class="btn-secondary"
            title="Centang hanya tabel yang berbeda di tabel utama"
            @click="handleSelectDiverged"
          >
            Pilih Tabel Beda Saja
          </button>
          <button
            v-if="divergedCount + remoteOnlyCount > 0"
            type="button"
            class="btn-primary"
            style="display: inline-flex; align-items: center; gap: 5px;"
            @click="handleSyncDiverged"
          >
            <Play :size="12" />
            <span>Sync Tabel Beda ({{ divergedCount + remoteOnlyCount }})</span>
          </button>
          <button type="button" class="btn-secondary" @click="$emit('close')">
            Tutup
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, onBeforeUnmount } from 'vue';
import {
  GitCompare,
  RotateCw,
  Search,
  Play,
} from 'lucide-vue-next';

const props = defineProps({
  diffList: {
    type: Array,
    default: () => [],
  },
  isLoading: {
    type: Boolean,
    default: false,
  },
  profileName: {
    type: String,
    default: '',
  },
});

const emit = defineEmits(['close', 'select-diverged', 'sync-diverged']);

const activeFilter = ref('all'); // 'all' | 'diverged' | 'in_sync'
const searchQuery = ref('');

const divergedCount = computed(() => {
  return props.diffList.filter((item) => item.status === 'diverged').length;
});

const remoteOnlyCount = computed(() => {
  return props.diffList.filter((item) => item.status === 'remote_only').length;
});

const inSyncCount = computed(() => {
  return props.diffList.filter((item) => item.status === 'in_sync').length;
});

const filteredList = computed(() => {
  let list = props.diffList;
  if (activeFilter.value === 'diverged') {
    list = list.filter((i) => i.status === 'diverged' || i.status === 'remote_only');
  } else if (activeFilter.value === 'in_sync') {
    list = list.filter((i) => i.status === 'in_sync');
  }

  const q = searchQuery.value.trim().toLowerCase();
  if (q) {
    list = list.filter((i) => i.table_name.toLowerCase().includes(q));
  }
  return list;
});

function getDivergedTableNames() {
  return props.diffList
    .filter((i) => i.status === 'diverged' || i.status === 'remote_only')
    .map((i) => i.table_name);
}

function handleSelectDiverged() {
  emit('select-diverged', getDivergedTableNames());
  emit('close');
}

function handleSyncDiverged() {
  emit('sync-diverged', getDivergedTableNames());
  emit('close');
}

function formatNumber(n) {
  const num = Number(n);
  if (!Number.isFinite(num)) return '0';
  return Math.abs(num).toString().replace(/\B(?=(\d{3})+(?!\d))/g, '.');
}

function onDiffKeyDown(e) {
  if (e.key === 'Escape') {
    emit('close');
  }
}

onMounted(() => {
  window.addEventListener('keydown', onDiffKeyDown);
});

onBeforeUnmount(() => {
  window.removeEventListener('keydown', onDiffKeyDown);
});
</script>

<style scoped>
.diff-modal-content {
  background: #ffffff;
  border-radius: var(--radius-xl);
  width: 720px;
  max-width: 95vw;
  max-height: 85vh;
  display: flex;
  flex-direction: column;
  box-shadow: 0 20px 25px -5px rgba(0, 0, 0, 0.2), 0 10px 10px -5px rgba(0, 0, 0, 0.08);
  overflow: hidden;
  animation: modalScale 0.2s cubic-bezier(0.16, 1, 0.3, 1);
}

@keyframes modalScale {
  from { opacity: 0; transform: scale(0.96); }
  to { opacity: 1; transform: scale(1); }
}

.diff-modal-header {
  padding: 1rem 1.25rem;
  border-bottom: 1px solid var(--border-color);
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.header-left {
  display: flex;
  align-items: center;
  gap: 0.625rem;
}

.header-icon {
  color: var(--primary-green);
}

.diff-title {
  font-size: 0.9375rem;
  font-weight: 700;
  color: #0f172a;
}

.diff-subtitle {
  font-size: 0.71875rem;
  color: #64748b;
}

.btn-close {
  background: transparent;
  border: 1px solid #e2e8f0;
  border-radius: var(--radius-sm);
  width: 26px;
  height: 26px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  color: #64748b;
  cursor: pointer;
  font-size: 11px;
}

.btn-close:hover {
  background: #f1f5f9;
  color: #0f172a;
}

.diff-loading-container {
  padding: 4rem 2rem;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 1rem;
}

.loading-icon {
  color: var(--primary-green);
}

.loading-text {
  font-size: 0.8125rem;
  color: #475569;
}

.diff-modal-body {
  padding: 1rem 1.25rem;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 0.875rem;
  flex: 1;
}

.metrics-grid {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 0.625rem;
}

.metric-card {
  background: #f8fafc;
  border: 1px solid #e2e8f0;
  border-radius: var(--radius-md);
  padding: 0.5rem 0.75rem;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.metric-label {
  font-size: 0.6875rem;
  color: #64748b;
}

.metric-val {
  font-size: 1.125rem;
  font-family: var(--font-mono);
  color: #0f172a;
}

.metric-diverged .metric-val { color: #d97706; }
.metric-new .metric-val { color: #2563eb; }
.metric-insync .metric-val { color: #059669; }

.diff-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
  flex-wrap: wrap;
}

.diff-tabs {
  display: flex;
  align-items: center;
  gap: 4px;
}

.diff-tab {
  background: transparent;
  border: 1px solid transparent;
  color: #64748b;
  font-size: 0.71875rem;
  font-weight: 500;
  padding: 3px 8px;
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition: all 0.15s ease;
}

.diff-tab:hover {
  background: #f1f5f9;
  color: #0f172a;
}

.diff-tab.active {
  background: #0f172a;
  color: #ffffff;
  font-weight: 600;
}

.diff-tab.tab-diverged { color: #d97706; }
.diff-tab.tab-diverged.active { background: #b45309; color: #ffffff; }

.diff-tab.tab-insync { color: #059669; }
.diff-tab.tab-insync.active { background: #065f46; color: #ffffff; }

.diff-search-box {
  position: relative;
  display: inline-flex;
  align-items: center;
  width: 160px;
}

.diff-search-icon {
  position: absolute;
  left: 6px;
  color: #94a3b8;
  pointer-events: none;
}

.diff-search-input {
  width: 100%;
  padding: 3px 18px 3px 22px;
  font-size: 0.71875rem;
  border: 1px solid #cbd5e1;
  border-radius: var(--radius-sm);
  outline: none;
}

.diff-search-input:focus {
  border-color: var(--primary-green);
}

.diff-search-clear {
  position: absolute;
  right: 4px;
  background: transparent;
  border: none;
  color: #94a3b8;
  font-size: 10px;
  cursor: pointer;
}

.diff-table-container {
  border: 1px solid var(--border-color);
  border-radius: var(--radius-md);
  max-height: 320px;
  overflow-y: auto;
  background: #ffffff;
}

.diff-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 0.75rem;
}

.diff-table th {
  position: sticky;
  top: 0;
  background: #f8fafc;
  padding: 6px 10px;
  border-bottom: 1px solid var(--border-color);
  color: #475569;
  font-weight: 600;
  font-size: 0.6875rem;
  text-align: left;
  z-index: 2;
}

.diff-table td {
  padding: 6px 10px;
  border-bottom: 1px solid #f1f5f9;
  color: #1e293b;
}

.diff-table tr:hover {
  background-color: #f8fafc;
}

.font-mono {
  font-family: var(--font-mono);
}

.col-diff-name {
  font-size: 0.71875rem;
  font-weight: 600;
}

.col-diff-num {
  font-size: 0.71875rem;
  color: #334155;
  font-variant-numeric: tabular-nums;
}

.diff-plus {
  color: #d97706;
  font-weight: 600;
}

.diff-minus {
  color: #dc2626;
  font-weight: 600;
}

.diff-zero {
  color: #94a3b8;
}

.diff-badge {
  display: inline-block;
  padding: 2px 6px;
  border-radius: 4px;
  font-size: 0.65625rem;
  font-weight: 600;
}

.badge-diverged {
  background: #fffbeb;
  color: #b45309;
  border: 1px solid #fde68a;
}

.badge-insync {
  background: #ecfdf5;
  color: #047857;
  border: 1px solid #a7f3d0;
}

.badge-new {
  background: #eff6ff;
  color: #1d4ed8;
  border: 1px solid #bfdbfe;
}

.badge-local {
  background: #f5f3ff;
  color: #6d28d9;
  border: 1px solid #ddd6fe;
}

.diff-empty-cell {
  padding: 2rem;
  text-align: center;
  color: #94a3b8;
  font-style: italic;
}

.diff-modal-footer {
  padding: 0.875rem 1.25rem;
  border-top: 1px solid var(--border-color);
  background: #f8fafc;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
  flex-wrap: wrap;
}

.diverged-hint {
  font-size: 0.71875rem;
  color: #475569;
}

.in-sync-hint {
  color: #059669;
  font-weight: 600;
}

.footer-right {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}
</style>
