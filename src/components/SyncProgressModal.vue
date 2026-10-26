<template>
  <div class="modal-overlay" @click.self="$emit('minimize')">
    <div class="modal-content">
      <div class="modal-header">
        <h2 class="modal-title">
          {{ isFinished ? resultTitle : `Syncing ${profileName}...` }}
        </h2>
        <button class="btn-secondary" style="padding: 0.25rem 0.5rem;" @click="$emit('minimize')">
          ✕
        </button>
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

        <!-- Terminal Console -->
        <div ref="terminalRef" class="terminal-console">
          <div
            v-for="(log, idx) in logs"
            :key="idx"
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
                @click="toggleLog(idx)"
              >
                {{ log.message }} {{ isLogExpanded(log, idx) ? '▾' : '▸' }}
              </button>
              <span v-else>{{ log.message }}</span>
            </span>
            <pre v-if="log.details && isLogExpanded(log, idx)" class="log-detail-content">{{ log.details }}</pre>
          </div>
          <div v-if="logs.length === 0" style="color: #64748b; font-style: italic;">
            Menunggu log dari engine sinkronisasi...
          </div>
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

defineEmits(['stop-sync', 'minimize', 'close']);

const terminalRef = ref(null);
const expandedLogs = ref(new Set());
const collapsedLogs = ref(new Set());
const currentTime = ref(Date.now());
const localStartedAt = Date.now();
let elapsedTimer;

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

function toggleLog(index) {
  const log = props.logs[index];
  const updated = new Set(log?.type === 'error' ? collapsedLogs.value : expandedLogs.value);
  const currentlyExpanded = isLogExpanded(log, index);

  if (log?.type === 'error') {
    if (currentlyExpanded) updated.add(index);
    else updated.delete(index);
    collapsedLogs.value = updated;
  } else {
    if (currentlyExpanded) updated.delete(index);
    else updated.add(index);
    expandedLogs.value = updated;
  }
}

function isLogExpanded(log, index) {
  if (log?.type === 'error') return !collapsedLogs.value.has(index);
  return expandedLogs.value.has(index);
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
});

onBeforeUnmount(() => {
  window.clearInterval(elapsedTimer);
});

watch(
  () => props.logs.length,
  async () => {
    await nextTick();
    if (terminalRef.value) {
      terminalRef.value.scrollTop = terminalRef.value.scrollHeight;
    }
  }
);
</script>
