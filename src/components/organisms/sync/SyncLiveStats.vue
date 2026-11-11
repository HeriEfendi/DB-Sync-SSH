<template>
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

    <div v-if="!isFinished" class="sync-activity" role="status" aria-live="polite">
      <span class="sync-activity-spinner"></span>
      <span><strong>Aktivitas:</strong> {{ currentActivity }}</span>
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
        <small v-if="progress.rows_synced_current_table">
          Tabel ini: {{ formatNumber(progress.rows_synced_current_table) }} baris
        </small>
      </div>
      <div v-if="globalFailureCount > 0" class="sync-live-stat">
        <span>Error koneksi/proses</span>
        <strong class="sync-stat-error">{{ formatNumber(globalFailureCount) }}</strong>
      </div>
    </div>
  </div>
</template>

<script setup>
import { formatNumber } from '../../../utils/formatters.js';

defineProps({
  progress: {
    type: Object,
    required: true,
  },
  percentComplete: {
    type: Number,
    default: 0,
  },
  isFinished: {
    type: Boolean,
    default: false,
  },
  currentActivity: {
    type: String,
    default: '',
  },
  elapsedLabel: {
    type: String,
    default: '00:00',
  },
  processedTableCount: {
    type: Number,
    default: 0,
  },
  successfulTableCount: {
    type: Number,
    default: 0,
  },
  failedTableCount: {
    type: Number,
    default: 0,
  },
  skippedTableCount: {
    type: Number,
    default: 0,
  },
  globalFailureCount: {
    type: Number,
    default: 0,
  },
});
</script>

