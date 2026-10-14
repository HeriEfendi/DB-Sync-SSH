<template>
  <div class="history-container">
    <header class="main-header" style="margin-bottom: 2rem;">
      <div class="header-title-group">
        <div style="display: flex; align-items: center; gap: 0.75rem;">
          <History :size="24" style="color: #f59e0b;" />
          <h1 class="main-title">Sync History</h1>
        </div>
      </div>
      <div class="header-actions">
        <button
          v-if="historyList.length > 0"
          class="btn-secondary"
          @click="$emit('clear-history')"
        >
          Clear History
        </button>
      </div>
    </header>

    <div class="history-list">
      <div
        v-for="item in historyList"
        :key="item.id"
        class="history-card"
      >
        <div class="history-icon-wrapper">
          <CheckCircle2
            v-if="item.status === 'Completed'"
            :size="26"
            style="color: var(--success-green);"
          />
          <AlertTriangle
            v-else-if="item.status === 'Aborted'"
            :size="26"
            style="color: var(--warning-amber);"
          />
          <XCircle
            v-else
            :size="26"
            style="color: var(--danger-red);"
          />
        </div>

        <div class="history-content">
          <div class="history-top-row">
            <h2 class="history-profile-title">{{ item.profile_name }}</h2>
            <div class="history-timestamp">
              <Clock :size="14" />
              <span>{{ item.timestamp }}</span>
            </div>
          </div>

          <div class="history-metrics-grid">
            <div class="metric-item">
              <span class="metric-label">Status</span>
              <span
                class="metric-value"
                :class="{
                  'status-completed': item.status === 'Completed',
                  'status-aborted': item.status === 'Aborted',
                  'status-failed': item.status === 'Failed'
                }"
              >
                {{ item.status }}
              </span>
            </div>

            <div class="metric-item">
              <span class="metric-label">Tables</span>
              <span class="metric-value">{{ item.tables_count }}</span>
            </div>

            <div class="metric-item">
              <span class="metric-label">Rows Copied</span>
              <span class="metric-value">{{ formatNumber(item.rows_copied) }}</span>
            </div>

            <div class="metric-item">
              <span class="metric-label">Duration</span>
              <span class="metric-value">{{ item.duration_text }}</span>
            </div>
          </div>
        </div>
      </div>

      <div
        v-if="historyList.length === 0"
        style="text-align: center; padding: 3rem; color: #94a3b8; background: #fff; border-radius: 8px; border: 1px solid #e2e8f0;"
      >
        Belum ada riwayat sinkronisasi. Jalankan sinkronisasi dari tab koneksi untuk melihat riwayat di sini.
      </div>
    </div>
  </div>
</template>

<script setup>
import { History, CheckCircle2, AlertTriangle, XCircle, Clock } from 'lucide-vue-next';

defineProps({
  historyList: {
    type: Array,
    required: true,
  },
});

defineEmits(['clear-history']);

function formatNumber(n) {
  if (n === undefined || n === null) return '0';
  return n.toString().replace(/\B(?=(\d{3})+(?!\d))/g, '.');
}
</script>
