<template>
  <div
    class="mini-floating-widget"
    :class="{
      'widget-finished': !isSyncing && result,
      'widget-success': !isSyncing && result?.status === 'success',
      'widget-partial': !isSyncing && result?.status === 'partial',
      'widget-failed': !isSyncing && (result?.status === 'failed' || result?.status === 'cancelled'),
    }"
    @mouseenter="onMouseEnter"
    @mouseleave="onMouseLeave"
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
        <!-- 5s Auto-dismiss counter badge -->
        <span
          v-if="!isSyncing && result"
          class="dismiss-counter-badge"
          :title="isHovered ? 'Hitung mundur ditahan selama kursor di sini' : `Menutup otomatis dalam ${dismissCountdown}s`"
        >
          {{ isHovered ? 'Dijeda' : `${dismissCountdown}s` }}
        </span>

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
          title="Tutup notifikasi ini sekarang"
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

    <!-- Auto-dismiss bar (after finish, shrinks over 5 seconds) -->
    <div v-else-if="result" class="dismiss-progress-track">
      <div class="dismiss-progress-fill" :style="{ width: ((dismissCountdown / 5) * 100) + '%' }"></div>
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
        <span v-if="!isSyncing && result" class="widget-dismiss-hint">
          {{ isHovered ? 'Arahkan kursor keluar untuk menutup' : `Menutup otomatis dalam ${dismissCountdown}s` }}
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
import { ref, computed, watch, onMounted, onBeforeUnmount } from 'vue';
import {
  CheckCircle2,
  AlertTriangle,
  XCircle,
  Maximize2,
  X,
  Square,
} from 'lucide-vue-next';
import { formatNumber } from '../utils/formatters.js';

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

const emit = defineEmits(['expand', 'stop-sync', 'dismiss']);

const currentTime = ref(Date.now());
let timer = null;
const dismissCountdown = ref(5);
let countdownInterval = null;
const isHovered = ref(false);

function startAutoDismiss() {
  clearAutoDismiss();
  if (props.isSyncing || !props.result) return;
  dismissCountdown.value = 5;

  countdownInterval = setInterval(() => {
    if (isHovered.value) return;
    dismissCountdown.value -= 1;
    if (dismissCountdown.value <= 0) {
      clearAutoDismiss();
      emit('dismiss');
    }
  }, 1000);
}

function clearAutoDismiss() {
  if (countdownInterval) {
    clearInterval(countdownInterval);
    countdownInterval = null;
  }
}

watch(
  () => !props.isSyncing && !!props.result,
  (isFinished) => {
    if (isFinished) {
      startAutoDismiss();
    } else {
      clearAutoDismiss();
    }
  },
  { immediate: true }
);

function onMouseEnter() {
  isHovered.value = true;
}

function onMouseLeave() {
  isHovered.value = false;
}

onMounted(() => {
  timer = setInterval(() => {
    currentTime.value = Date.now();
  }, 500);
});

onBeforeUnmount(() => {
  clearInterval(timer);
  clearAutoDismiss();
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
</script>

<style scoped>
/* Scoped styles migrated to src/styles/widget.css */
</style>
