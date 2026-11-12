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
            type="button"
            class="modal-btn-header"
            title="Minimize ke widget mengambang"
            @click="$emit('minimize')"
          >
            <Minus :size="14" />
            <span class="btn-label-hide-sm"></span>
          </button>
          <button
            type="button"
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

        <!-- Live Stats & Progress Bar Component -->
        <SyncLiveStats
          :progress="progress"
          :percent-complete="percentComplete"
          :is-finished="isFinished"
          :current-activity="currentActivity"
          :elapsed-label="elapsedLabel"
          :processed-table-count="processedTableCount"
          :successful-table-count="successfulTableCount"
          :failed-table-count="failedTableCount"
          :skipped-table-count="skippedTableCount"
          :global-failure-count="globalFailureCount"
        />

        <!-- Terminal Console Component -->
        <SyncTerminalLog
          ref="terminalLogRef"
          :logs="logs"
          :profile-name="profileName"
        />

        <!-- Failures Accordion -->
        <div v-if="result?.failures?.length > 0" class="failures-summary-container">
          <div class="failures-header" @click="showFailures = !showFailures">
            <AlertTriangle :size="14" class="text-error" />
            <span>{{ result.failures.length }} Error Kegagalan Terjadi</span>
            <span class="failures-toggle-icon">{{ showFailures ? '▲' : '▼' }}</span>
          </div>
          <div v-if="showFailures" class="failures-list">
            <div
              v-for="(fail, fIdx) in result.failures"
              :key="fIdx"
              class="failure-row"
            >
              <span class="failure-stage">{{ fail.table_name ? `[${fail.table_name}] ` : '' }}{{ fail.stage }}:</span>
              <span class="failure-msg">{{ fail.message }}</span>
            </div>
          </div>
        </div>
      </div>

      <!-- Modal Footer -->
      <div class="modal-footer">
        <button
          v-if="!isFinished"
          type="button"
          class="btn-danger-solid"
          :disabled="isCancelling"
          @click="$emit('stop-sync')"
        >
          <Square :size="12" />
          <span>{{ isCancelling ? 'Membatalkan...' : 'Hentikan Sinkronisasi' }}</span>
        </button>
        <button
          v-if="isFinished"
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
import { ref, computed, onMounted, onBeforeUnmount } from 'vue';
import { Minus, X, AlertTriangle, Square } from 'lucide-vue-next';
import { formatDurationMs } from '../utils/formatters.js';
import SyncLiveStats from './organisms/sync/SyncLiveStats.vue';
import SyncTerminalLog from './organisms/sync/SyncTerminalLog.vue';

const props = defineProps({
  profileName: { type: String, default: '' },
  progress: { type: Object, required: true },
  logs: { type: Array, default: () => [] },
  startedAt: { type: Number, default: 0 },
  isFinished: { type: Boolean, default: false },
  result: { type: Object, default: null },
  isCancelling: { type: Boolean, default: false },
});

const emit = defineEmits(['stop-sync', 'minimize', 'close']);

const showFailures = ref(false);
const terminalLogRef = ref(null);
const currentTime = ref(Date.now());
let timer = null;

onMounted(() => {
  timer = setInterval(() => { currentTime.value = Date.now(); }, 500);
  window.addEventListener('keydown', onKeyDown);
});

onBeforeUnmount(() => {
  clearInterval(timer);
  window.removeEventListener('keydown', onKeyDown);
});

function onKeyDown(e) {
  if (e.key === 'Escape') {
    if (props.isFinished) emit('close');
    else emit('minimize');
  } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'f') {
    e.preventDefault();
    terminalLogRef.value?.focusSearch();
  } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 's') {
    e.preventDefault();
    terminalLogRef.value?.exportLogs();
  }
}

const percentComplete = computed(() => {
  if (!props.progress.total_tables) return 0;
  const processed = props.progress.processed_tables ?? props.progress.completed_tables ?? 0;
  return Math.min(100, Math.max(0, Math.round((processed / props.progress.total_tables) * 100)));
});

const elapsedMilliseconds = computed(() => {
  if (props.result?.duration_ms) return props.result.duration_ms;
  if (props.isFinished && props.progress.elapsed_ms) return props.progress.elapsed_ms;
  if (!props.startedAt) return 0;
  return Math.max(0, currentTime.value - props.startedAt);
});

const elapsedLabel = computed(() => formatDurationMs(elapsedMilliseconds.value));

const currentActivity = computed(() => {
  const latest = [...props.logs].reverse().find((l) => l.type === 'info' || l.type === 'success');
  return latest?.message || 'Menyiapkan sinkronisasi...';
});

const processedTableCount = computed(() => {
  return props.progress.processed_tables ?? props.progress.completed_tables ?? 0;
});

const successfulTableCount = computed(() => props.progress.successful_tables ?? 0);
const failedTableCount = computed(() => props.progress.failed_tables ?? 0);
const skippedTableCount = computed(() => props.progress.skipped_tables ?? 0);
const globalFailureCount = computed(() => (props.result?.failures || []).filter((f) => !f.table_name).length);

const resultTitle = computed(() => {
  if (!props.result) return 'Sync Selesai';
  if (props.result.status === 'success') return 'Sinkronisasi Selesai Sukses';
  if (props.result.status === 'partial') return 'Sinkronisasi Selesai Sebagian';
  if (props.result.status === 'cancelled') return 'Sinkronisasi Dibatalkan';
  return 'Sinkronisasi Gagal';
});

const resultMessage = computed(() => {
  if (!props.result) return '';
  return props.result.status === 'success'
    ? `Seluruh ${props.result.total_tables || 0} tabel berhasil disinkronkan (${props.result.total_rows_synced || 0} baris).`
    : `Sinkronisasi berakhir dengan status '${props.result.status}'. Silakan cek rekaman log di bawah.`;
});
</script>

<style scoped>
/* Modular styles imported in src/styles/sync-modal.css */
</style>
