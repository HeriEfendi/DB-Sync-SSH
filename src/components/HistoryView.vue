<template>
  <div class="history-container">
    <header class="main-header">
      <div class="header-title-group">
        <div style="display: flex; align-items: center; gap: 0.5rem;">
          <History :size="16" style="color: var(--primary-green);" />
          <h1 class="main-title">Sync History</h1>
          <span v-if="historyList.length > 0" class="history-count-badge">
            {{ historyList.length }} runs
          </span>
        </div>
      </div>
      <div class="header-actions">
        <button
          v-if="historyList.length > 0"
          class="btn-secondary"
          title="Clear all sync history"
          @click="$emit('clear-history')"
        >
          <Trash2 :size="13" />
          <span>Clear History</span>
        </button>
      </div>
    </header>

    <div class="table-card history-table-card">
      <div class="table-scroll-container">
        <table class="data-table">
          <thead>
            <tr>
              <th style="width: 130px;">Status</th>
              <th>Connection</th>
              <th style="width: 90px; text-align: right;">Tables</th>
              <th style="width: 130px; text-align: right;">Rows Copied</th>
              <th style="width: 100px; text-align: right;">Duration</th>
              <th style="width: 170px; text-align: right;">Date & Time</th>
              <th style="width: 80px; text-align: center;">Detail</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="item in historyList"
              :key="item.id"
              class="history-row"
              :class="{ 'row-active': selectedItem?.id === item.id }"
              @click="selectedItem = item"
            >
              <td>
                <span
                  class="status-badge"
                  :class="{
                    completed: item.status === 'Completed',
                    partial: item.status === 'Partial',
                    aborted: item.status === 'Aborted',
                    cancelled: item.status === 'Cancelled',
                    failed: item.status === 'Failed',
                  }"
                >
                  <CheckCircle2 v-if="item.status === 'Completed'" :size="12" />
                  <AlertTriangle v-else-if="['Aborted', 'Partial', 'Cancelled'].includes(item.status)" :size="12" />
                  <XCircle v-else :size="12" />
                  <span>{{ item.status }}</span>
                </span>
              </td>
              <td>
                <div class="history-profile-cell">
                  <Database :size="13" class="history-db-icon" />
                  <span class="history-profile-name">{{ item.profile_name }}</span>
                </div>
              </td>
              <td style="text-align: right; font-family: var(--font-mono); font-size: 12px; font-variant-numeric: tabular-nums;">
                {{ item.tables_count }}
              </td>
              <td style="text-align: right; font-family: var(--font-mono); font-size: 12px; font-weight: 600; font-variant-numeric: tabular-nums;">
                {{ formatNumber(item.rows_copied) }}
              </td>
              <td style="text-align: right; font-family: var(--font-mono); font-size: 12px; color: #475569;">
                {{ item.duration_text }}
              </td>
              <td style="text-align: right; font-family: var(--font-mono); font-size: 11.5px; color: #64748b;">
                {{ item.timestamp }}
              </td>
              <td style="text-align: center;">
                <button
                  type="button"
                  class="btn-detail-link"
                  title="Lihat detail riwayat"
                  @click.stop="selectedItem = item"
                >
                  <Eye :size="12" />
                  <span>Buka</span>
                </button>
              </td>
            </tr>

            <tr v-if="historyList.length === 0">
              <td colspan="7" class="empty-state-cell">
                Belum ada riwayat sinkronisasi. Jalankan sinkronisasi dari tab koneksi untuk melihat riwayat di sini.
              </td>
            </tr>
          </tbody>
        </table>
      </div>

      <div v-if="historyList.length > 0" class="table-footer-status">
        <span>Total {{ historyList.length }} riwayat sinkronisasi</span>
        <span>Total baris tersalin: {{ formatNumber(totalRowsCopied) }}</span>
      </div>
    </div>

    <!-- Side Drawer: History Detail Panel -->
    <div
      v-if="selectedItem"
      class="drawer-backdrop"
      @click.self="selectedItem = null"
    >
      <div class="drawer-panel">
        <div class="drawer-header">
          <div class="drawer-title-group">
            <h2 class="drawer-title">Detail Riwayat Sync</h2>
            <span class="drawer-subtitle">{{ selectedItem.timestamp }}</span>
          </div>
          <button
            type="button"
            class="drawer-close-btn"
            title="Tutup (Esc)"
            @click="selectedItem = null"
          >
            ✕
          </button>
        </div>

        <div class="drawer-body">
          <!-- Summary Cards -->
          <div class="drawer-stats-grid">
            <div class="stat-box">
              <span class="stat-label">Profil Database</span>
              <strong class="stat-val">{{ selectedItem.profile_name }}</strong>
            </div>
            <div class="stat-box">
              <span class="stat-label">Status</span>
              <span
                class="status-badge"
                :class="{
                  completed: selectedItem.status === 'Completed',
                  partial: selectedItem.status === 'Partial',
                  aborted: selectedItem.status === 'Aborted',
                  cancelled: selectedItem.status === 'Cancelled',
                  failed: selectedItem.status === 'Failed',
                }"
              >
                {{ selectedItem.status }}
              </span>
            </div>
            <div class="stat-box">
              <span class="stat-label">Mode Sinkronisasi</span>
              <strong class="stat-val">{{ selectedItem.mode === 'fresh' ? 'Fresh Sync' : 'Sync New & Update' }}</strong>
            </div>
            <div class="stat-box">
              <span class="stat-label">Durasi</span>
              <strong class="stat-val font-mono">{{ selectedItem.duration_text }}</strong>
            </div>
            <div class="stat-box">
              <span class="stat-label">Baris Tersalin</span>
              <strong class="stat-val font-mono" style="color: var(--primary-green);">{{ formatNumber(selectedItem.rows_copied) }}</strong>
            </div>
            <div class="stat-box">
              <span class="stat-label">Jumlah Tabel</span>
              <strong class="stat-val font-mono">{{ selectedItem.tables_count }}</strong>
            </div>
          </div>

          <!-- Failures section if any -->
          <div v-if="selectedItem.failures && selectedItem.failures.length > 0" class="drawer-section">
            <div class="section-badge-error">
              <AlertTriangle :size="13" />
              <span>Catatan / Error yang Terjadi ({{ selectedItem.failures.length }})</span>
            </div>
            <div class="failures-list">
              <div
                v-for="(fail, fIdx) in selectedItem.failures"
                :key="fIdx"
                class="failure-card"
              >
                <div class="failure-title">
                  <span v-if="fail.table_name" class="fail-tbl">[{{ fail.table_name }}]</span>
                  <span>{{ fail.stage }}</span>
                </div>
                <div class="failure-msg">{{ fail.message }}</div>
              </div>
            </div>
          </div>

          <!-- Sync Logs -->
          <div class="drawer-section">
            <div class="log-section-header" @click="logsExpanded = !logsExpanded">
              <div class="log-section-title">
                <AlertCircle :size="13" />
                <span>Log Sinkronisasi{{ selectedItemLogs.length > 0 ? ` (${selectedItemLogs.length})` : '' }}</span>
                <span class="log-count-pill error" v-if="errorLogCount > 0">{{ errorLogCount }} error</span>
                <span class="log-count-pill warn" v-if="warnLogCount > 0">{{ warnLogCount }} warn</span>
              </div>
              <span class="log-toggle-icon">{{ logsExpanded ? '▲' : '▼' }}</span>
            </div>

            <div v-if="logsExpanded">
              <!-- No logs stored (old entry) -->
              <div v-if="selectedItemLogs.length === 0" class="log-empty-state">
                <span>Log tidak tersedia untuk riwayat ini.</span>
                <span class="log-empty-hint">Log hanya tersimpan untuk sinkronisasi yang dijalankan setelah versi ini.</span>
              </div>

              <!-- Log list -->
              <div v-else class="log-list">
                <div
                  v-for="(log, lIdx) in selectedItemLogs"
                  :key="lIdx"
                  class="log-entry"
                  :class="log.type"
                >
                  <div class="log-entry-header">
                    <span class="log-type-badge" :class="log.type">
                      {{
                        log.type === 'error' ? '✕ ERROR'
                        : log.type === 'warn' || log.type === 'warning' ? '⚠ WARN'
                        : log.type === 'success' ? '✓ OK'
                        : 'ℹ INFO'
                      }}
                    </span>
                    <span v-if="log.table_name" class="log-table-tag">[{{ log.table_name }}]</span>
                    <span v-if="log.timestamp" class="log-timestamp">{{ log.timestamp }}</span>
                  </div>
                  <div class="log-message">{{ log.message }}</div>
                  <div v-if="log.details" class="log-details">{{ log.details }}</div>
                </div>
              </div>
            </div>
          </div>

          <!-- Synced Tables List -->
          <div class="drawer-section">
            <div class="section-header-row">
              <div class="section-title">
                <Database :size="13" />
                <span>Daftar Tabel ({{ selectedItem.table_names?.length || selectedItem.tables_count || 0 }})</span>
              </div>
              <input
                v-if="selectedItem.table_names && selectedItem.table_names.length > 5"
                v-model="tableFilterQuery"
                type="text"
                placeholder="Cari tabel..."
                class="drawer-search-input"
              />
            </div>

            <div class="drawer-tables-scroll">
              <div
                v-for="tName in filteredDrawerTables"
                :key="tName"
                class="drawer-table-row"
              >
                <span class="tbl-bullet">•</span>
                <span class="tbl-name">{{ tName }}</span>
              </div>
              <div v-if="filteredDrawerTables.length === 0" class="drawer-table-empty">
                Tidak ada data tabel tersimpan untuk riwayat ini.
              </div>
            </div>
          </div>
        </div>

        <div class="drawer-footer">
          <button
            type="button"
            class="btn-primary"
            style="display: inline-flex; align-items: center; gap: 6px;"
            @click="triggerRerun"
          >
            <RotateCw :size="13" />
            <span>Jalankan Ulang Sync Ini</span>
          </button>
          <button
            type="button"
            class="btn-secondary"
            @click="selectedItem = null"
          >
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
  History,
  CheckCircle2,
  AlertTriangle,
  AlertCircle,
  XCircle,
  Database,
  Trash2,
  Eye,
  RotateCw,
} from 'lucide-vue-next';

const props = defineProps({
  historyList: {
    type: Array,
    required: true,
  },
});

const emit = defineEmits(['clear-history', 'rerun-sync']);

const selectedItem = ref(null);
const tableFilterQuery = ref('');
const logsExpanded = ref(true);

// Computed: logs for currently selected history item
const selectedItemLogs = computed(() => {
  return selectedItem.value?.logs || [];
});

const errorLogCount = computed(() =>
  selectedItemLogs.value.filter((l) => l.type === 'error').length
);

const warnLogCount = computed(() =>
  selectedItemLogs.value.filter((l) => l.type === 'warn' || l.type === 'warning').length
);

const totalRowsCopied = computed(() => {
  return props.historyList.reduce((sum, item) => sum + (Number(item.rows_copied) || 0), 0);
});

const filteredDrawerTables = computed(() => {
  const list = selectedItem.value?.table_names || [];
  if (!tableFilterQuery.value.trim()) return list;
  const q = tableFilterQuery.value.trim().toLowerCase();
  return list.filter((t) => t.toLowerCase().includes(q));
});

function triggerRerun() {
  if (!selectedItem.value) return;
  const item = selectedItem.value;
  selectedItem.value = null;
  emit('rerun-sync', item);
}

function formatNumber(n) {
  if (n === undefined || n === null) return '0';
  return n.toString().replace(/\B(?=(\d{3})+(?!\d))/g, '.');
}

function onHistoryKeyDown(e) {
  if (e.key === 'Escape' && selectedItem.value) {
    selectedItem.value = null;
  }
}

onMounted(() => {
  window.addEventListener('keydown', onHistoryKeyDown);
});

onBeforeUnmount(() => {
  window.removeEventListener('keydown', onHistoryKeyDown);
});
</script>

<style scoped>
.history-row {
  cursor: pointer;
  transition: background-color 0.15s ease;
}

.history-row:hover {
  background-color: #f1f5f9 !important;
}

.history-row.row-active {
  background-color: var(--primary-green-light) !important;
}

.btn-detail-link {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  padding: 2px 7px;
  font-size: 0.6875rem;
  border-radius: var(--radius-sm);
  border: 1px solid #cbd5e1;
  background: #ffffff;
  color: #475569;
  cursor: pointer;
  transition: all 0.15s ease;
}

.btn-detail-link:hover {
  border-color: var(--primary-green);
  color: var(--primary-green);
  background-color: var(--primary-green-light);
}

/* Side Drawer Overlay */
.drawer-backdrop {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(15, 23, 42, 0.4);
  backdrop-filter: blur(4px);
  -webkit-backdrop-filter: blur(4px);
  z-index: 1200;
  display: flex;
  justify-content: flex-end;
  animation: fadeIn 0.2s ease;
}

@keyframes fadeIn {
  from { opacity: 0; }
  to { opacity: 1; }
}

.drawer-panel {
  width: 560px;
  max-width: 90vw;
  height: 100%;
  background: #ffffff;
  box-shadow: -8px 0 25px rgba(0, 0, 0, 0.15);
  display: flex;
  flex-direction: column;
  animation: slideDrawer 0.25s cubic-bezier(0.16, 1, 0.3, 1);
}

@keyframes slideDrawer {
  from { transform: translateX(100%); }
  to { transform: translateX(0); }
}

.drawer-header {
  padding: 1rem 1.25rem;
  border-bottom: 1px solid var(--border-color);
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.drawer-title-group {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.drawer-title {
  font-size: 0.9375rem;
  font-weight: 700;
  color: #0f172a;
}

.drawer-subtitle {
  font-size: 0.6875rem;
  color: #64748b;
  font-family: var(--font-mono);
}

.drawer-close-btn {
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

.drawer-close-btn:hover {
  background: #f1f5f9;
  color: #0f172a;
}

.drawer-body {
  flex: 1;
  overflow-y: auto;
  padding: 1.25rem;
  display: flex;
  flex-direction: column;
  gap: 1.25rem;
}

.drawer-stats-grid {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 0.625rem;
}

.stat-box {
  background: #f8fafc;
  border: 1px solid #e2e8f0;
  border-radius: var(--radius-md);
  padding: 0.625rem 0.75rem;
  display: flex;
  flex-direction: column;
  gap: 3px;
}

.stat-label {
  font-size: 0.6875rem;
  color: #64748b;
}

.stat-val {
  font-size: 0.8125rem;
  color: #0f172a;
}

.font-mono {
  font-family: var(--font-mono);
}

.drawer-section {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.section-badge-error {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  background: #fef2f2;
  color: #b91c1c;
  border: 1px solid #fecaca;
  padding: 4px 8px;
  border-radius: var(--radius-sm);
  font-size: 0.71875rem;
  font-weight: 600;
}

.failures-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.failure-card {
  background: #fff5f5;
  border: 1px solid #fed7d7;
  border-radius: var(--radius-sm);
  padding: 6px 10px;
  font-size: 0.71875rem;
}

.failure-title {
  font-weight: 600;
  color: #c53030;
  display: flex;
  gap: 4px;
}

.fail-tbl {
  font-family: var(--font-mono);
}

.failure-msg {
  color: #742a2a;
  font-family: var(--font-mono);
  font-size: 0.6875rem;
  margin-top: 2px;
  word-break: break-all;
}

.section-header-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
}

.section-title {
  display: flex;
  align-items: center;
  gap: 5px;
  font-size: 0.78125rem;
  font-weight: 600;
  color: #0f172a;
}

.drawer-search-input {
  width: 130px;
  padding: 2px 6px;
  font-size: 0.6875rem;
  border: 1px solid #cbd5e1;
  border-radius: var(--radius-sm);
  outline: none;
}

.drawer-search-input:focus {
  border-color: var(--primary-green);
}

.drawer-tables-scroll {
  border: 1px solid #e2e8f0;
  border-radius: var(--radius-md);
  background: #f8fafc;
  max-height: 220px;
  overflow-y: auto;
  padding: 6px;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.drawer-table-row {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 3px 6px;
  border-radius: 4px;
  font-family: var(--font-mono);
  font-size: 0.71875rem;
  color: #1e293b;
}

.drawer-table-row:hover {
  background: #ffffff;
}

.tbl-bullet {
  color: var(--primary-green);
}

.tbl-name {
  word-break: break-all;
}

.drawer-table-empty {
  padding: 1rem;
  text-align: center;
  color: #94a3b8;
  font-size: 0.71875rem;
  font-style: italic;
}

.drawer-footer {
  padding: 1rem 1.25rem;
  border-top: 1px solid var(--border-color);
  background: #f8fafc;
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 0.625rem;
}

/* ── Log Section ── */
.log-section-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 5px 8px;
  border-radius: var(--radius-sm);
  background: #f1f5f9;
  border: 1px solid #e2e8f0;
  cursor: pointer;
  user-select: none;
  transition: background 0.15s ease;
}

.log-section-header:hover {
  background: #e8eef5;
}

.log-section-title {
  display: flex;
  align-items: center;
  gap: 5px;
  font-size: 0.78125rem;
  font-weight: 600;
  color: #0f172a;
}

.log-toggle-icon {
  font-size: 0.6rem;
  color: #94a3b8;
}

.log-count-pill {
  display: inline-flex;
  align-items: center;
  padding: 1px 6px;
  border-radius: 99px;
  font-size: 0.625rem;
  font-weight: 700;
}

.log-count-pill.error {
  background: #fee2e2;
  color: #b91c1c;
}

.log-count-pill.warn {
  background: #fef3c7;
  color: #92400e;
}

.log-list {
  display: flex;
  flex-direction: column;
  gap: 5px;
  max-height: 260px;
  overflow-y: auto;
  padding: 2px 0;
}

.log-entry {
  border-radius: var(--radius-sm);
  padding: 6px 8px;
  font-size: 0.71875rem;
  border-left: 3px solid transparent;
  background: #f8fafc;
  border: 1px solid #e2e8f0;
}

.log-entry.error {
  background: #fff5f5;
  border-color: #fecaca;
  border-left-color: #ef4444;
}

.log-entry.warn,
.log-entry.warning {
  background: #fffbeb;
  border-color: #fde68a;
  border-left-color: #f59e0b;
}

.log-entry.info {
  background: #eff6ff;
  border-color: #bfdbfe;
  border-left-color: #3b82f6;
}

.log-entry-header {
  display: flex;
  align-items: center;
  gap: 5px;
  flex-wrap: wrap;
  margin-bottom: 3px;
}

.log-type-badge {
  font-size: 0.5625rem;
  font-weight: 800;
  letter-spacing: 0.04em;
  padding: 1px 5px;
  border-radius: 3px;
  white-space: nowrap;
}

.log-type-badge.error {
  background: #fecaca;
  color: #991b1b;
}

.log-type-badge.warn,
.log-type-badge.warning {
  background: #fde68a;
  color: #78350f;
}

.log-type-badge.info {
  background: #bfdbfe;
  color: #1e40af;
}

.log-type-badge.success {
  background: #dcfce7;
  color: #15803d;
}

.log-table-tag {
  font-family: var(--font-mono);
  font-size: 0.6875rem;
  color: #475569;
  font-weight: 600;
}

.log-timestamp {
  margin-left: auto;
  font-family: var(--font-mono);
  font-size: 0.625rem;
  color: #94a3b8;
  white-space: nowrap;
}

.log-message {
  color: #1e293b;
  line-height: 1.45;
  word-break: break-word;
}

.log-details {
  margin-top: 4px;
  font-family: var(--font-mono);
  font-size: 0.625rem;
  color: #64748b;
  word-break: break-all;
  white-space: pre-wrap;
  background: rgba(0,0,0,0.03);
  padding: 3px 5px;
  border-radius: 3px;
  line-height: 1.5;
}

.log-entry.success {
  background: #f0fdf4;
  border-color: #bbf7d0;
  border-left-color: #22c55e;
}

.log-empty-state {
  display: flex;
  flex-direction: column;
  gap: 3px;
  padding: 10px 10px;
  border: 1px dashed #e2e8f0;
  border-radius: var(--radius-sm);
  color: #94a3b8;
  font-size: 0.71875rem;
  font-style: italic;
}

.log-empty-hint {
  font-size: 0.625rem;
  color: #cbd5e1;
}
</style>

