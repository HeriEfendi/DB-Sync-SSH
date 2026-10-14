<template>
  <div class="history-container">
    <header class="main-header">
      <div class="header-title-group">
        <div style="display: flex; align-items: center; gap: 0.5rem;">
          <History :size="16" style="color: #f59e0b;" />
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
            </tr>
          </thead>
          <tbody>
            <tr v-for="item in historyList" :key="item.id">
              <td>
                <span
                  class="status-badge"
                  :class="{
                    completed: item.status === 'Completed',
                    aborted: item.status === 'Aborted',
                    failed: item.status === 'Failed',
                  }"
                >
                  <CheckCircle2 v-if="item.status === 'Completed'" :size="12" />
                  <AlertTriangle v-else-if="item.status === 'Aborted'" :size="12" />
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
            </tr>

            <tr v-if="historyList.length === 0">
              <td colspan="6" class="empty-state-cell">
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
  </div>
</template>

<script setup>
import { computed } from 'vue';
import {
  History,
  CheckCircle2,
  AlertTriangle,
  XCircle,
  Database,
  Trash2,
} from 'lucide-vue-next';

const props = defineProps({
  historyList: {
    type: Array,
    required: true,
  },
});

defineEmits(['clear-history']);

const totalRowsCopied = computed(() => {
  return props.historyList.reduce((sum, item) => sum + (Number(item.rows_copied) || 0), 0);
});

function formatNumber(n) {
  if (n === undefined || n === null) return '0';
  return n.toString().replace(/\B(?=(\d{3})+(?!\d))/g, '.');
}
</script>
