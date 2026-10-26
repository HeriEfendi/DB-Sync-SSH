<template>
  <div
    class="mini-floating-widget"
    :class="{
      'widget-finished': !isSyncing && result,
      'widget-success': !isSyncing && result?.status === 'success',
      'widget-partial': !isSyncing && result?.status === 'partial',
      'widget-failed': !isSyncing && (result?.status === 'failed' || result?.status === 'cancelled'),
    }"
  >
    <!-- Top Row: Status Icon, Title, Actions -->
    <div class="widget-header">
      <div class="widget-status-badge">
        <span v-if="isSyncing" class="widget-spinner-dot"></span>
        <CheckCircle2 v-else-if="result?.status === 'success'" :size="15" class="icon-success" />
        <AlertTriangle v-else-if="result?.status === 'partial'" :size="15" class="icon-warning" />
        <XCircle v-else :size="15" class="icon-error" />
        <span class="widget-title" :title="profileName">
          {{ isSyncing ? `Syncing: ${profileName}` : (result?.status === 'success' ? 'Sync Selesai' : 'Sync Selesai (Catatan)') }}
        </span>
      </div>

      <div class="widget-actions">
        <button
          type="button"
          class="widget-btn-icon"
          title="Buka tampilan log penuh"
          @click="$emit('expand')"
        >
          <Maximize2 :size="13" />
        </button>
        <button
          v-if="isSyncing"
          type="button"
          class="widget-btn-icon widget-btn-stop"
          :title="isCancelling ? 'Membatalkan...' : 'Hentikan proses sync'"
          :disabled="isCancelling"
          @click="$emit('stop-sync')"
        >
          <Square :size="11" />
        </button>
        <button
          v-else
          type="button"
          class="widget-btn-icon"
          title="Tutup notifikasi ini"
          @click="$emit('dismiss')"
        >
          <X :size="13" />
        </button>
      </div>
    </div>

    <!-- Progress Bar (while syncing) -->
    <div v-if="isSyncing" class="widget-progress-track">
      <div class="widget-progress-fill" :style="{ width: percentComplete + '%' }"></div>
    </div>

    <!-- Body Info -->
    <div class="widget-body">
      <div v-if="isSyncing" class="widget-info-row">
        <span class="widget-current-table" :title="progress.current_table_name">
          Tabel: <strong>{{ progress.current_table_name || 'Menyiapkan...' }}</strong>
        </span>
        <span class="widget-progress-text">
          {{ progress.current_table_index || 0 }}/{{ progress.total_tables || 0 }} ({{ percentComplete }}%)
        </span>
      </div>
      <div v-else class="widget-info-row">
        <span class="widget-result-summary">
          {{ formatNumber(result?.total_rows_synced || progress.total_synced_all_tables || 0) }} baris tersalin
        </span>
        <span class="widget-duration">
          {{ elapsedLabel }}
        </span>
      </div>

      <div class="widget-footer-row">
        <span v-if="isSyncing" class="widget-stats-pill">
          ⚡ {{ formatNumber(progress.total_synced_all_tables || 0) }} baris
        </span>
        <span v-if="isSyncing" class="widget-time-pill">
          ⏱️ {{ elapsedLabel }}
        </span>
        <button
          v-if="!isSyncing"
          type="button"
          class="widget-link-btn"
          @click="$emit('expand')"
        >
          Lihat Log Lengkap →
        </button>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, onBeforeUnmount } from 'vue';
import {
  CheckCircle2,
  AlertTriangle,
  XCircle,
  Maximize2,
  X,
  Square,
} from 'lucide-vue-next';

const props = defineProps({
  profileName: { type: String, default: '' },
  progress: {
    type: Object,
    default: () => ({
      current_table_index: 0,
      total_tables: 0,
      current_table_name: '',
      rows_synced_current_table: 0,
      total_synced_all_tables: 0,
      processed_tables: 0,
      successful_tables: 0,
      failed_tables: 0,
      skipped_tables: 0,
      elapsed_ms: 0,
    }),
  },
  isSyncing: { type: Boolean, default: false },
  isCancelling: { type: Boolean, default: false },
  result: { type: Object, default: null },
  startedAt: { type: Number, default: 0 },
});

defineEmits(['expand', 'stop-sync', 'dismiss']);

const currentTime = ref(Date.now());
let timer;

onMounted(() => {
  timer = setInterval(() => {
    currentTime.value = Date.now();
  }, 500);
});

onBeforeUnmount(() => {
  clearInterval(timer);
});

const percentComplete = computed(() => {
  const total = props.progress.total_tables;
  if (!total) return 0;
  const processed = props.progress.processed_tables ?? props.progress.completed_tables ?? 0;
  return Math.min(100, Math.max(0, Math.round((processed / total) * 100)));
});

const elapsedMilliseconds = computed(() => {
  if (props.result?.duration_ms) return props.result.duration_ms;
  if (!props.isSyncing && props.progress.elapsed_ms) return props.progress.elapsed_ms;
  if (!props.startedAt) return 0;
  return Math.max(0, currentTime.value - props.startedAt);
});

const elapsedLabel = computed(() => {
  const totalSeconds = Math.floor(Math.max(0, elapsedMilliseconds.value) / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${minutes}m ${String(seconds).padStart(2, '0')}s`;
});

function formatNumber(n) {
  const value = Number(n);
  if (!Number.isFinite(value)) return '0';
  return Math.trunc(value).toString().replace(/\B(?=(\d{3})+(?!\d))/g, '.');
}
</script>

<style scoped>
.mini-floating-widget {
  position: fixed;
  bottom: 1.5rem;
  right: 1.5rem;
  width: 330px;
  background: rgba(255, 255, 255, 0.96);
  backdrop-filter: blur(10px);
  -webkit-backdrop-filter: blur(10px);
  border: 1px solid #cbd5e1;
  border-radius: 10px;
  box-shadow: 0 10px 25px -5px rgba(15, 23, 42, 0.15), 0 8px 10px -6px rgba(15, 23, 42, 0.1);
  z-index: 1050;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  animation: slideUpFade 0.25s cubic-bezier(0.16, 1, 0.3, 1);
  font-family: var(--font-sans);
}

@keyframes slideUpFade {
  from {
    opacity: 0;
    transform: translateY(16px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

.widget-success {
  border-color: #a7f3d0;
  background: #f0fdf4;
}

.widget-partial {
  border-color: #fde68a;
  background: #fffbeb;
}

.widget-failed {
  border-color: #fecaca;
  background: #fef2f2;
}

.widget-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0.625rem 0.75rem 0.5rem;
  gap: 0.5rem;
}

.widget-status-badge {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  min-width: 0;
  flex: 1;
}

.widget-spinner-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background-color: #10b981;
  animation: pulseGreen 1.5s infinite;
  display: inline-block;
  flex-shrink: 0;
}

@keyframes pulseGreen {
  0% { transform: scale(0.95); box-shadow: 0 0 0 0 rgba(16, 185, 129, 0.7); }
  70% { transform: scale(1); box-shadow: 0 0 0 6px rgba(16, 185, 129, 0); }
  100% { transform: scale(0.95); box-shadow: 0 0 0 0 rgba(16, 185, 129, 0); }
}

.icon-success { color: #059669; flex-shrink: 0; }
.icon-warning { color: #d97706; flex-shrink: 0; }
.icon-error { color: #dc2626; flex-shrink: 0; }

.widget-title {
  font-size: 0.78125rem;
  font-weight: 600;
  color: #0f172a;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.widget-actions {
  display: flex;
  align-items: center;
  gap: 4px;
}

.widget-btn-icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  border-radius: 4px;
  border: 1px solid transparent;
  background: transparent;
  color: #64748b;
  cursor: pointer;
  transition: all 0.15s ease;
}

.widget-btn-icon:hover {
  background: #e2e8f0;
  color: #0f172a;
}

.widget-btn-stop:hover {
  background: #fee2e2;
  color: #dc2626;
  border-color: #fca5a5;
}

.widget-progress-track {
  width: 100%;
  height: 3px;
  background-color: #e2e8f0;
  overflow: hidden;
}

.widget-progress-fill {
  height: 100%;
  background: linear-gradient(90deg, #10b981, #059669);
  transition: width 0.3s ease;
}

.widget-body {
  padding: 0.5rem 0.75rem 0.625rem;
  display: flex;
  flex-direction: column;
  gap: 0.375rem;
}

.widget-info-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
  font-size: 0.75rem;
}

.widget-current-table {
  font-family: var(--font-mono);
  font-size: 0.71875rem;
  color: #334155;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 200px;
}

.widget-current-table strong {
  color: #0e7548;
}

.widget-progress-text {
  font-family: var(--font-mono);
  font-size: 0.6875rem;
  font-weight: 600;
  color: #475569;
  flex-shrink: 0;
}

.widget-result-summary {
  font-size: 0.75rem;
  font-weight: 600;
  color: #0f172a;
}

.widget-duration {
  font-family: var(--font-mono);
  font-size: 0.6875rem;
  color: #64748b;
}

.widget-footer-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
  font-size: 0.6875rem;
}

.widget-stats-pill,
.widget-time-pill {
  font-family: var(--font-mono);
  font-size: 0.6875rem;
  color: #475569;
  background: #f1f5f9;
  padding: 2px 6px;
  border-radius: 4px;
}

.widget-link-btn {
  background: transparent;
  border: none;
  color: #0e7548;
  font-size: 0.71875rem;
  font-weight: 600;
  cursor: pointer;
  padding: 0;
  margin-left: auto;
}

.widget-link-btn:hover {
  text-decoration: underline;
}
</style>
