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
          type="button"
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
              <th style="width: 170px; text-align: right;">Date &amp; Time</th>
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
    </div>

    <!-- Detail Drawer Sub-component -->
    <HistoryDetailDrawer
      :item="selectedItem"
      @close="selectedItem = null"
      @rerun="handleRerun"
    />
  </div>
</template>

<script setup>
import { ref, onMounted, onBeforeUnmount } from 'vue';
import {
  History,
  Trash2,
  CheckCircle2,
  AlertTriangle,
  XCircle,
  Database,
  Eye,
} from 'lucide-vue-next';
import { formatNumber } from '../utils/formatters.js';
import HistoryDetailDrawer from './organisms/history/HistoryDetailDrawer.vue';

defineProps({
  historyList: {
    type: Array,
    default: () => [],
  },
});

const emit = defineEmits(['clear-history', 'rerun-sync']);

const selectedItem = ref(null);

function handleRerun(item) {
  selectedItem.value = null;
  emit('rerun-sync', item);
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
/* Modular styles imported in src/styles/history.css */
</style>
