<template>
  <div class="modal-overlay" @click.self="$emit('minimize')">
    <div class="modal-content">
      <!-- Modal Header -->
      <div class="modal-header">
        <div class="modal-title-group">
          <h2 class="modal-title">
            {{ isFinished ? resultTitle : `Syncing ${profileName}...` }}
          </h2>
          <span v-if="!isFinished" class="modal-badge-running">
            <span class="live-pulse-dot"></span> Sedang Berjalan
          </span>
        </div>
        <div class="modal-header-actions">
          <button
            class="modal-btn-header"
            title="Minimize ke widget mengambang"
            @click="$emit('minimize')"
          >
            <Minus :size="14" />
            <span class="btn-label-hide-sm">Minimize</span>
          </button>
          <button
            class="modal-btn-header"
            :title="isFinished ? 'Tutup modal' : 'Minimize modal'"
            @click="$emit('minimize')"
          >
            <X :size="14" />
          </button>
        </div>
      </div>

      <div class="modal-body">
        <div
          v-if="isFinished && result"
          class="sync-result-banner"
          :class="`sync-result-${result.status || 'failed'}`"
        >
          {{ resultMessage }}
        </div>

        <!-- Progress Bar -->
        <div class="progress-container">
          <div class="progress-header">
            <span>
              Tabel {{ progress.current_table_index || 0 }} dari {{ progress.total_tables || 0 }}:
              <strong style="color: var(--primary-green)">{{ progress.current_table_name || '-' }}</strong>
            </span>
            <span>{{ percentComplete }}%</span>
          </div>

          <div class="progress-bar-track">
            <div class="progress-bar-fill" :style="{ width: percentComplete + '%' }"></div>
          </div>

          <div class="sync-live-stats">
            <div class="sync-live-stat">
              <span>Durasi berjalan</span>
              <strong>{{ elapsedLabel }}</strong>
            </div>
            <div class="sync-live-stat">
              <span>Tabel diproses</span>
              <strong>{{ formatNumber(processedTableCount) }} / {{ formatNumber(progress.total_tables) }}</strong>
            </div>
            <div class="sync-live-stat">
              <span>Berhasil</span>
              <strong class="sync-stat-success">{{ formatNumber(successfulTableCount) }}</strong>
            </div>
            <div class="sync-live-stat">
              <span>Gagal</span>
              <strong :class="{ 'sync-stat-error': failedTableCount > 0 }">{{ formatNumber(failedTableCount) }}</strong>
            </div>
            <div class="sync-live-stat">
              <span>Dilewati</span>
              <strong>{{ formatNumber(skippedTableCount) }}</strong>
            </div>
            <div class="sync-live-stat sync-live-stat-rows">
              <span>Baris terkirim ke MySQL lokal</span>
              <strong>{{ formatNumber(progress.total_synced_all_tables) }}</strong>
              <small>Tabel ini: {{ formatNumber(progress.rows_synced_current_table) }} baris</small>
            </div>
            <div v-if="globalFailureCount > 0" class="sync-live-stat">
              <span>Error koneksi/proses</span>
              <strong class="sync-stat-error">{{ formatNumber(globalFailureCount) }}</strong>
            </div>
          </div>
        </div>

        <!-- Terminal Console Toolbar (Filter Tabs & Search & Copy) -->
        <div class="terminal-toolbar">
          <div class="terminal-tabs">
            <button
              type="button"
              class="terminal-tab"
              :class="{ active: activeLogFilter === 'all' }"
              @click="activeLogFilter = 'all'"
            >
              Semua ({{ logs.length }})
            </button>
            <button
              v-if="errorLogCount > 0"
              type="button"
              class="terminal-tab tab-error"
              :class="{ active: activeLogFilter === 'error' }"
              @click="activeLogFilter = 'error'"
            >
              Error ({{ errorLogCount }})
            </button>
            <button
              v-if="warningLogCount > 0"
              type="button"
              class="terminal-tab tab-warning"
              :class="{ active: activeLogFilter === 'warning' }"
              @click="activeLogFilter = 'warning'"
            >
              Warning ({{ warningLogCount }})
            </button>
            <button
              v-if="successLogCount > 0"
              type="button"
              class="terminal-tab tab-success"
              :class="{ active: activeLogFilter === 'success' }"
              @click="activeLogFilter = 'success'"
            >
              Berhasil ({{ successLogCount }})
            </button>
          </div>

          <div class="terminal-actions">
            <!-- Search within logs -->
            <div class="terminal-search-box">
              <Search :size="12" class="terminal-search-icon" />
              <input
                ref="logSearchInputRef"
                v-model="logSearchQuery"
                type="text"
                placeholder="Cari log... (Ctrl+F)"
                class="terminal-search-input"
              />
              <button
                v-if="logSearchQuery"
                type="button"
                class="terminal-search-clear"
                title="Hapus pencarian"
                @click="logSearchQuery = ''"
              >
                ✕
              </button>
            </div>

            <!-- Copy logs button -->
            <button
              type="button"
              class="terminal-action-btn"
              :title="copiedLogs ? 'Berhasil disalin ke clipboard!' : 'Salin seluruh log ke clipboard'"
              @click="copyAllLogs"
            >
              <Check v-if="copiedLogs" :size="12" style="color: #10b981;" />
              <Copy v-else :size="12" />
              <span>{{ copiedLogs ? 'Tersalin!' : 'Salin Log' }}</span>
            </button>

            <!-- Export logs button -->
            <button
              type="button"
              class="terminal-action-btn"
              :title="exportedLogs ? 'Berhasil diekspor ke file!' : 'Ekspor log ke file .txt (Ctrl+S)'"
              @click="exportLogsToFile"
            >
              <Check v-if="exportedLogs" :size="12" style="color: #10b981;" />
              <Download v-else :size="12" />
              <span>{{ exportedLogs ? 'Terekspor!' : 'Ekspor .txt' }}</span>
            </button>
          </div>
        </div>

        <!-- Terminal Console Container with Scroll-lock and Jump-to-bottom button -->
        <div class="terminal-wrapper">
          <div
            ref="terminalRef"
            class="terminal-console"
            @scroll="onTerminalScroll"
          >
            <div
              v-for="(log, idx) in filteredLogs"
              :key="getLogKey(log, idx)"
              class="log-entry"
            >
              <span class="log-time">[{{ log.timestamp }}]</span>
              <span
                :class="{
                  'log-msg-info': log.type === 'info',
                  'log-msg-success': log.type === 'success',
                  'log-msg-warning': log.type === 'warning' || log.type === 'warn',
                  'log-msg-error': log.type === 'error'
                }"
              >
                <button
                  v-if="log.details"
                  type="button"
                  class="log-detail-toggle"
                  :class="{
                    'log-msg-info': log.type === 'info',
                    'log-msg-success': log.type === 'success',
                    'log-msg-warning': log.type === 'warning' || log.type === 'warn',
                    'log-msg-error': log.type === 'error'
                  }"
                  :aria-expanded="isLogExpanded(log, idx)"
                  @click="toggleLog(log, idx)"
                >
                  {{ log.message }} {{ isLogExpanded(log, idx) ? '▾' : '▸' }}
                </button>
                <span v-else>{{ log.message }}</span>
              </span>
              <pre v-if="log.details && isLogExpanded(log, idx)" class="log-detail-content">{{ log.details }}</pre>
            </div>
            <div v-if="filteredLogs.length === 0" class="log-empty-state">
              <span v-if="logSearchQuery || activeLogFilter !== 'all'">
                Tidak ada log yang cocok dengan filter "{{ activeLogFilter !== 'all' ? activeLogFilter : logSearchQuery }}".
              </span>
              <span v-else style="color: #64748b; font-style: italic;">
                Menunggu log dari engine sinkronisasi...
              </span>
            </div>
          </div>

          <!-- Floating button to jump down if user scrolled up -->
          <button
            v-if="!isAutoScrollEnabled && logs.length > 0"
            type="button"
            class="terminal-scroll-bottom-btn"
            title="Klik untuk kembali ke log paling bawah"
            @click="scrollToBottom"
          >
            <ArrowDown :size="12" />
            <span>Scroll ke bawah</span>
          </button>
        </div>
      </div>

      <div class="modal-footer">
        <button
          v-if="!isFinished"
          type="button"
          class="btn-danger"
          :disabled="isCancelling"
          @click="$emit('stop-sync')"
        >
          {{ isCancelling ? 'Membatalkan...' : 'Stop Sync' }}
        </button>
        <button
          v-else
          type="button"
          class="btn-primary"
          @click="$emit('close')"
        >
          Tutup
        </button>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, watch, nextTick, onMounted, onBeforeUnmount } from 'vue';
import {
  Minus,
  X,
  Search,
  Copy,
  Check,
  ArrowDown,
  Download,
} from 'lucide-vue-next';

const props = defineProps({
  profileName: {
    type: String,
    default: '',
  },
  progress: {
    type: Object,
    default: () => ({
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
    }),
  },
  startedAt: {
    type: Number,
    default: 0,
  },
  logs: {
    type: Array,
    default: () => [],
  },
  result: {
    type: Object,
    default: null,
  },
  isCancelling: {
    type: Boolean,
    default: false,
  },
  isFinished: {
    type: Boolean,
    default: false,
  },
});

const emit = defineEmits(['stop-sync', 'minimize', 'close']);

const terminalRef = ref(null);
const logSearchInputRef = ref(null);
const expandedLogs = ref(new Set());
const collapsedLogs = ref(new Set());
const currentTime = ref(Date.now());
const localStartedAt = Date.now();
let elapsedTimer;

const activeLogFilter = ref('all'); // 'all' | 'error' | 'warning' | 'success'
const logSearchQuery = ref('');
const copiedLogs = ref(false);
const exportedLogs = ref(false);
const isAutoScrollEnabled = ref(true);

const errorLogCount = computed(() => props.logs.filter((l) => l.type === 'error').length);
const warningLogCount = computed(() => props.logs.filter((l) => l.type === 'warning' || l.type === 'warn').length);
const successLogCount = computed(() => props.logs.filter((l) => l.type === 'success').length);

const filteredLogs = computed(() => {
  let list = props.logs;
  if (activeLogFilter.value === 'error') {
    list = list.filter((l) => l.type === 'error');
  } else if (activeLogFilter.value === 'warning') {
    list = list.filter((l) => l.type === 'warning' || l.type === 'warn');
  } else if (activeLogFilter.value === 'success') {
    list = list.filter((l) => l.type === 'success');
  }

  const q = logSearchQuery.value.trim().toLowerCase();
  if (q) {
    list = list.filter((l) =>
      (l.message && l.message.toLowerCase().includes(q)) ||
      (l.details && l.details.toLowerCase().includes(q)) ||
      (l.table_name && l.table_name.toLowerCase().includes(q))
    );
  }
  return list;
});

function getLogKey(log, idx) {
  return `${log.timestamp}-${log.type}-${log.message?.substring(0, 20)}-${idx}`;
}

const resultTitle = computed(() => {
  switch (props.result?.status) {
    case 'success': return 'Sinkronisasi Berhasil';
    case 'partial': return 'Sinkronisasi Sebagian';
    case 'cancelled': return 'Sinkronisasi Dibatalkan';
    default: return 'Sinkronisasi Gagal';
  }
});

const resultMessage = computed(() => {
  if (props.result?.error) return props.result.error;
  if (props.result?.status === 'success') return 'Semua tabel berhasil diproses.';
  return 'Sinkronisasi tidak selesai sepenuhnya.';
});

const failedTableNames = computed(() => {
  const names = new Set();
  for (const log of props.logs) {
    if (log.type === 'error' && log.table_name) names.add(log.table_name);
  }
  for (const failure of props.result?.failures || []) {
    if (failure.table_name) names.add(failure.table_name);
  }
  return names;
});

const completedTableNames = computed(() => {
  const names = props.progress.completed_table_names;
  return new Set(Array.isArray(names) ? names : []);
});

const skippedTableNames = computed(() => {
  const names = props.progress.skipped_table_names;
  return new Set(Array.isArray(names) ? names : []);
});

const processedTableCount = computed(() => {
  if (completedTableNames.value.size === 0 && !failedTableNames.value.size) {
    return props.progress.processed_tables ?? props.progress.completed_tables ?? 0;
  }
  return new Set([...completedTableNames.value, ...failedTableNames.value]).size;
});

const successfulTableCount = computed(() => {
  if (completedTableNames.value.size === 0 && props.progress.successful_tables) {
    return props.progress.successful_tables;
  }
  return [...completedTableNames.value].filter(
    (name) => !failedTableNames.value.has(name) && !skippedTableNames.value.has(name)
  ).length;
});

const failedTableCount = computed(() => {
  return Math.max(failedTableNames.value.size, props.progress.failed_tables || 0);
});

const skippedTableCount = computed(() => {
  const skippedCount = [...skippedTableNames.value].filter(
    (name) => !failedTableNames.value.has(name)
  ).length;
  return Math.max(skippedCount, props.progress.skipped_tables || 0);
});

const globalFailureCount = computed(() => {
  const failures = new Set();
  for (const log of props.logs) {
    if (log.type === 'error' && !log.table_name) {
      const stage = log.message.replace(/ gagal$/, '');
      failures.add(`${stage}:${log.details || ''}`);
    }
  }
  for (const failure of props.result?.failures || []) {
    if (!failure.table_name) failures.add(`${failure.stage}:${failure.message}`);
  }
  return failures.size;
});

const elapsedMilliseconds = computed(() => {
  const finalDuration = props.result?.duration_ms ?? (props.isFinished ? props.progress.elapsed_ms : null);
  if (finalDuration !== null && finalDuration !== undefined) return finalDuration;
  return Math.max(0, currentTime.value - (props.startedAt || localStartedAt));
});

const elapsedLabel = computed(() => formatDuration(elapsedMilliseconds.value));

function toggleLog(log, idx) {
  const key = getLogKey(log, idx);
  const isErr = log?.type === 'error';
  const updated = new Set(isErr ? collapsedLogs.value : expandedLogs.value);
  const currentlyExpanded = isLogExpanded(log, idx);

  if (isErr) {
    if (currentlyExpanded) updated.add(key);
    else updated.delete(key);
    collapsedLogs.value = updated;
  } else {
    if (currentlyExpanded) updated.delete(key);
    else updated.add(key);
    expandedLogs.value = updated;
  }
}

function isLogExpanded(log, idx) {
  const key = getLogKey(log, idx);
  if (log?.type === 'error') return !collapsedLogs.value.has(key);
  return expandedLogs.value.has(key);
}

function onTerminalScroll() {
  if (!terminalRef.value) return;
  const { scrollTop, scrollHeight, clientHeight } = terminalRef.value;
  // If user scrolled up more than 35px from bottom, pause auto-scroll
  isAutoScrollEnabled.value = scrollHeight - scrollTop - clientHeight < 35;
}

function scrollToBottom() {
  if (terminalRef.value) {
    terminalRef.value.scrollTop = terminalRef.value.scrollHeight;
    isAutoScrollEnabled.value = true;
  }
}

async function copyAllLogs() {
  if (!props.logs || props.logs.length === 0) return;
  const text = props.logs
    .map((l) => {
      let str = `[${l.timestamp}] [${(l.type || 'INFO').toUpperCase()}] ${l.message}`;
      if (l.details) {
        str += `\n  ${l.details.split('\n').join('\n  ')}`;
      }
      return str;
    })
    .join('\n');

  try {
    if (navigator.clipboard && navigator.clipboard.writeText) {
      await navigator.clipboard.writeText(text);
    } else {
      const ta = document.createElement('textarea');
      ta.value = text;
      document.body.appendChild(ta);
      ta.select();
      document.execCommand('copy');
      document.body.removeChild(ta);
    }
    copiedLogs.value = true;
    setTimeout(() => {
      copiedLogs.value = false;
    }, 2000);
  } catch (err) {
    console.error('Failed to copy logs:', err);
  }
}

function exportLogsToFile() {
  if (!props.logs || props.logs.length === 0) return;
  const header =
    `========================================================\n` +
    ` DB-Sync-Ssh - Synchronization Log Export\n` +
    ` Profile   : ${props.profileName}\n` +
    ` Exported  : ${new Date().toLocaleString('id-ID')}\n` +
    ` Status    : ${props.result?.status || (props.isFinished ? 'Finished' : 'Running')}\n` +
    ` Tables    : ${processedTableCount.value}/${props.progress.total_tables || 0}\n` +
    ` Rows      : ${formatNumber(props.progress.total_synced_all_tables || 0)}\n` +
    ` Duration  : ${elapsedLabel.value}\n` +
    `========================================================\n\n`;

  const body = props.logs
    .map((l) => {
      let str = `[${l.timestamp}] [${(l.type || 'INFO').toUpperCase()}] ${l.message}`;
      if (l.details) {
        str += `\n  Details:\n  ${l.details.split('\n').join('\n  ')}`;
      }
      return str;
    })
    .join('\n');

  const fullContent = header + body;
  const blob = new Blob([fullContent], { type: 'text/plain;charset=utf-8' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  const safeProfile = (props.profileName || 'sync').toLowerCase().replace(/[^a-z0-9]/g, '_');
  const timestamp = new Date().toISOString().replace(/[:.]/g, '-').slice(0, 19);
  a.href = url;
  a.download = `db_sync_${safeProfile}_${timestamp}.txt`;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);

  exportedLogs.value = true;
  setTimeout(() => {
    exportedLogs.value = false;
  }, 2000);
}

function onModalKeyDown(e) {
  if (e.key === 'Escape') {
    if (props.isFinished) {
      emit('close');
    } else {
      emit('minimize');
    }
  } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'f') {
    e.preventDefault();
    if (logSearchInputRef.value) {
      logSearchInputRef.value.focus();
      logSearchInputRef.value.select();
    }
  } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 's') {
    e.preventDefault();
    exportLogsToFile();
  }
}

const percentComplete = computed(() => {
  if (!props.progress.total_tables) return 0;
  const p = Math.round((processedTableCount.value / props.progress.total_tables) * 100);
  return Math.min(100, Math.max(0, p));
});

function formatNumber(n) {
  const value = Number(n);
  if (!Number.isFinite(value)) return '0';
  return Math.trunc(value).toString().replace(/\B(?=(\d{3})+(?!\d))/g, '.');
}

function formatDuration(milliseconds) {
  const totalSeconds = Math.floor(Math.max(0, milliseconds) / 1000);
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  return [hours, minutes, seconds].map((value) => String(value).padStart(2, '0')).join(':');
}

onMounted(() => {
  elapsedTimer = window.setInterval(() => {
    currentTime.value = Date.now();
  }, 250);
  window.addEventListener('keydown', onModalKeyDown);
});

onBeforeUnmount(() => {
  window.clearInterval(elapsedTimer);
  window.removeEventListener('keydown', onModalKeyDown);
});

watch(
  () => props.logs.length,
  async () => {
    if (isAutoScrollEnabled.value) {
      await nextTick();
      if (terminalRef.value) {
        terminalRef.value.scrollTop = terminalRef.value.scrollHeight;
      }
    }
  }
);
</script>

<style scoped>
.modal-title-group {
  display: flex;
  align-items: center;
  gap: 0.75rem;
}

.modal-badge-running {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  background-color: var(--primary-green-light);
  color: var(--primary-green);
  border: 1px solid rgba(var(--primary-green-rgb), 0.2);
  padding: 2px 8px;
  border-radius: 9999px;
  font-size: 0.6875rem;
  font-weight: 600;
}

.live-pulse-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background-color: #10b981;
  animation: pulseGreen 1.5s infinite;
}

@keyframes pulseGreen {
  0% { transform: scale(0.95); box-shadow: 0 0 0 0 rgba(16, 185, 129, 0.7); }
  70% { transform: scale(1); box-shadow: 0 0 0 6px rgba(16, 185, 129, 0); }
  100% { transform: scale(0.95); box-shadow: 0 0 0 0 rgba(16, 185, 129, 0); }
}

.modal-header-actions {
  display: flex;
  align-items: center;
  gap: 6px;
}

.modal-btn-header {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  background-color: #f1f5f9;
  border: 1px solid #cbd5e1;
  border-radius: var(--radius-sm);
  padding: 0.3125rem 0.5rem;
  font-size: 0.71875rem;
  font-weight: 500;
  color: #475569;
  cursor: pointer;
  transition: all 0.15s ease;
}

.modal-btn-header:hover {
  background-color: #e2e8f0;
  color: #0f172a;
}

@media (max-width: 600px) {
  .btn-label-hide-sm {
    display: none;
  }
}

/* Terminal Toolbar */
.terminal-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
  margin-top: 1rem;
  margin-bottom: 0.375rem;
  flex-wrap: wrap;
}

.terminal-tabs {
  display: flex;
  align-items: center;
  gap: 4px;
}

.terminal-tab {
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

.terminal-tab:hover {
  background: #f1f5f9;
  color: #0f172a;
}

.terminal-tab.active {
  background: #0b1120;
  color: #f8fafc;
  font-weight: 600;
}

.terminal-tab.tab-error {
  color: #ef4444;
}

.terminal-tab.tab-error.active {
  background: #991b1b;
  color: #ffffff;
}

.terminal-tab.tab-warning {
  color: #d97706;
}

.terminal-tab.tab-warning.active {
  background: #b45309;
  color: #ffffff;
}

.terminal-tab.tab-success {
  color: #059669;
}

.terminal-tab.tab-success.active {
  background: #065f46;
  color: #ffffff;
}

.terminal-actions {
  display: flex;
  align-items: center;
  gap: 6px;
}

.terminal-search-box {
  position: relative;
  display: inline-flex;
  align-items: center;
  width: 140px;
}

.terminal-search-icon {
  position: absolute;
  left: 6px;
  color: #94a3b8;
  pointer-events: none;
}

.terminal-search-input {
  width: 100%;
  padding: 3px 18px 3px 22px;
  font-size: 0.71875rem;
  border: 1px solid #cbd5e1;
  border-radius: var(--radius-sm);
  background: #ffffff;
  color: #0f172a;
  outline: none;
}

.terminal-search-input:focus {
  border-color: var(--primary-green);
}

.terminal-search-clear {
  position: absolute;
  right: 4px;
  background: transparent;
  border: none;
  color: #94a3b8;
  font-size: 10px;
  cursor: pointer;
  padding: 2px;
}

.terminal-action-btn {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 3px 8px;
  font-size: 0.71875rem;
  font-weight: 500;
  border: 1px solid #cbd5e1;
  background: #ffffff;
  color: #475569;
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition: all 0.15s ease;
}

.terminal-action-btn:hover {
  background: #f8fafc;
  color: #0f172a;
  border-color: #94a3b8;
}

/* Terminal Wrapper */
.terminal-wrapper {
  position: relative;
  width: 100%;
}

.terminal-scroll-bottom-btn {
  position: absolute;
  bottom: 12px;
  right: 18px;
  display: inline-flex;
  align-items: center;
  gap: 4px;
  background: #1e293b;
  color: #f8fafc;
  border: 1px solid #475569;
  border-radius: 9999px;
  padding: 4px 10px;
  font-size: 0.6875rem;
  font-weight: 600;
  cursor: pointer;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.25);
  animation: bounceSoft 1s infinite alternate;
  z-index: 10;
}

.terminal-scroll-bottom-btn:hover {
  background: #334155;
}

@keyframes bounceSoft {
  from { transform: translateY(0); }
  to { transform: translateY(-3px); }
}

.log-empty-state {
  padding: 2rem 1rem;
  text-align: center;
  color: #94a3b8;
  font-size: 0.78125rem;
  font-style: italic;
}
</style>
