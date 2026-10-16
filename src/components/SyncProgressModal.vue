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
              <strong style="color: var(--primary-blue)">{{ progress.current_table_name || '-' }}</strong>
            </span>
            <span>{{ percentComplete }}%</span>
          </div>

          <div class="progress-bar-track">
            <div class="progress-bar-fill" :style="{ width: percentComplete + '%' }"></div>
          </div>

          <div style="display: flex; justify-content: space-between; font-size: 0.75rem; color: #64748b;">
            <span>Baris tabel ini: {{ formatNumber(progress.rows_synced_current_table) }}</span>
            <span>Total baris: <strong>{{ formatNumber(progress.total_synced_all_tables) }}</strong></span>
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
import { ref, computed, watch, nextTick } from 'vue';

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
      current_table_name: '',
      rows_synced_current_table: 0,
      total_synced_all_tables: 0,
      status: 'idle',
    }),
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
  const p = Math.round((props.progress.current_table_index / props.progress.total_tables) * 100);
  return Math.min(100, Math.max(0, p));
});

function formatNumber(n) {
  if (n === undefined || n === null) return '0';
  return n.toString().replace(/\B(?=(\d{3})+(?!\d))/g, '.');
}

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
