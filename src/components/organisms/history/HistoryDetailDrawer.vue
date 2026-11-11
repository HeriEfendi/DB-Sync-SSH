<template>
  <div v-if="item" class="drawer-backdrop" @click.self="$emit('close')">
    <div class="drawer-panel">
      <!-- Drawer Header -->
      <div class="drawer-header">
        <div class="drawer-title-group">
          <div style="display: flex; align-items: center; gap: 0.5rem;">
            <h3 class="drawer-title">Detail Riwayat Sync</h3>
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
              {{ item.status }}
            </span>
          </div>
          <span class="drawer-subtitle">{{ item.profile_name }} • {{ item.timestamp }}</span>
        </div>
        <button type="button" class="drawer-close-btn" title="Tutup (Esc)" @click="$emit('close')">
          ✕
        </button>
      </div>

      <!-- Drawer Body -->
      <div class="drawer-body">
        <!-- Summary Stats Grid -->
        <div class="drawer-stats-grid">
          <div class="stat-box">
            <span class="stat-label">Mode Sinkronisasi</span>
            <strong class="stat-val" style="text-transform: capitalize;">
              {{ item.mode === 'fresh' ? 'Fresh Sync' : 'Sync New & Update' }}
            </strong>
          </div>
          <div class="stat-box">
            <span class="stat-label">Limit Baris</span>
            <strong class="stat-val font-mono">
              {{ item.row_limit ? formatNumber(item.row_limit) : 'Unlimited' }}
            </strong>
          </div>
          <div class="stat-box">
            <span class="stat-label">Total Tabel</span>
            <strong class="stat-val font-mono">{{ item.tables_count }}</strong>
          </div>
          <div class="stat-box">
            <span class="stat-label">Baris Tersalin</span>
            <strong class="stat-val font-mono" style="color: var(--primary-green); font-weight: 700;">
              {{ formatNumber(item.rows_copied) }}
            </strong>
          </div>
          <div class="stat-box">
            <span class="stat-label">Durasi Waktu</span>
            <strong class="stat-val font-mono">{{ item.duration_text }}</strong>
          </div>
          <div class="stat-box">
            <span class="stat-label">ID Riwayat</span>
            <strong class="stat-val font-mono" style="font-size: 11px; color: #64748b;">
              {{ item.id }}
            </strong>
          </div>
        </div>

        <!-- Section: Problematic / Structure-changed tables -->
        <div v-if="problematicTables.length > 0" class="drawer-section">
          <div class="drawer-section-title">
            <span>Tabel Butuh Perhatian ({{ problematicTables.length }})</span>
          </div>
          <div class="tables-summary-list">
            <div
              v-for="tbl in problematicTables"
              :key="tbl.name"
              class="table-summary-item"
              :class="`item-${tbl.status}`"
            >
              <span class="tbl-name">{{ tbl.name }}</span>
              <span :class="['tbl-badge', tbl.badgeClass]">{{ tbl.badge }}</span>
            </div>
          </div>
        </div>

        <!-- Section: Tabs (Logs / Failures) -->
        <div class="drawer-section">
          <div class="drawer-tabs">
            <button
              type="button"
              class="drawer-tab"
              :class="{ active: detailTab === 'logs' }"
              @click="detailTab = 'logs'"
            >
              <Terminal :size="13" />
              <span>Log Terminal</span>
              <span v-if="item.logs?.length" class="tab-badge">{{ item.logs.length }}</span>
            </button>
            <button
              v-if="item.failures?.length"
              type="button"
              class="drawer-tab"
              :class="{ active: detailTab === 'failures' }"
              @click="detailTab = 'failures'"
            >
              <AlertCircle :size="13" />
              <span>Daftar Error</span>
              <span class="tab-badge badge-error">{{ item.failures.length }}</span>
            </button>
          </div>

          <!-- Tab Content: Logs -->
          <div v-if="detailTab === 'logs'" class="drawer-tab-content">
            <div class="toolbar-row">
              <div class="search-mini">
                <Search :size="11" class="search-mini-icon" />
                <input
                  v-model="logSearchQuery"
                  type="text"
                  placeholder="Filter log..."
                />
              </div>
              <select v-model="logFilterType" class="filter-select-mini">
                <option value="all">Semua Tipe</option>
                <option value="error">Error Saja</option>
                <option value="warning">Warning Saja</option>
                <option value="success">Success Saja</option>
              </select>
              <button
                type="button"
                class="btn-secondary btn-copy-logs"
                title="Salin seluruh log ke clipboard"
                @click="copyLogsToClipboard"
              >
                <Copy :size="11" />
                <span>{{ copiedLogs ? 'Tersalin!' : 'Copy' }}</span>
              </button>
            </div>

            <div class="drawer-log-box">
              <div
                v-for="(log, idx) in filteredDetailLogs"
                :key="idx"
                class="log-entry"
                :class="log.type"
              >
                <span class="log-time">{{ log.timestamp }}</span>
                <span>{{ log.message }}</span>
                <span v-if="log.details" style="display: block; opacity: 0.8; font-size: 10px;">
                  ↳ {{ log.details }}
                </span>
              </div>
              <div v-if="filteredDetailLogs.length === 0" class="empty-logs">
                Tidak ada log yang cocok dengan filter.
              </div>
            </div>
          </div>

          <!-- Tab Content: Failures -->
          <div v-else-if="detailTab === 'failures'" class="drawer-tab-content">
            <div class="failures-accordion">
              <div
                v-for="(fail, idx) in item.failures"
                :key="idx"
                class="failure-item"
              >
                <div class="failure-header">
                  <AlertTriangle :size="13" />
                  <span>{{ fail.table_name ? `[${fail.table_name}] ` : '' }}{{ fail.stage }}</span>
                </div>
                <div class="failure-details">{{ fail.message }}</div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- Drawer Footer Action -->
      <div class="drawer-footer">
        <span style="font-size: 0.6875rem; color: #64748b;">
          {{ item.failed_tables?.length || item.failures?.length ? 'Terdapat tabel gagal yang bisa diulang' : 'Semua proses tuntas' }}
        </span>
        <button
          type="button"
          class="btn-rerun-sync"
          title="Jalankan ulang sinkronisasi untuk konfigurasi ini"
          @click="$emit('rerun', item)"
        >
          <RotateCcw :size="12" />
          <span>Sync Ulang</span>
        </button>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, computed } from 'vue';
import {
  Terminal,
  AlertCircle,
  Search,
  Copy,
  AlertTriangle,
  RotateCcw,
} from 'lucide-vue-next';
import { formatNumber, extractTableName } from '../../../utils/formatters.js';

const props = defineProps({
  item: {
    type: Object,
    default: null,
  },
});

defineEmits(['close', 'rerun']);

const detailTab = ref('logs');
const logSearchQuery = ref('');
const logFilterType = ref('all');
const copiedLogs = ref(false);

const problematicTables = computed(() => {
  if (!props.item) return [];
  const result = [];
  const failedList = props.item.failed_tables || (props.item.failures || []).map((f) => f.table_name).filter(Boolean);
  const changedList = props.item.structure_changed_tables || [];
  const failedSet = new Set(failedList);

  for (const name of failedList) {
    result.push({
      name,
      status: 'failed',
      badge: '✕ Gagal',
      badgeClass: 'badge-failed',
    });
  }
  for (const name of changedList) {
    if (!failedSet.has(name)) {
      result.push({
        name,
        status: 'structure_drift',
        badge: '⚠️ Perubahan Struktur',
        badgeClass: 'badge-changed',
      });
    }
  }
  return result;
});

const filteredDetailLogs = computed(() => {
  if (!props.item?.logs) return [];
  return props.item.logs.filter((log) => {
    if (logFilterType.value !== 'all' && log.type !== logFilterType.value) return false;
    if (logSearchQuery.value.trim()) {
      const q = logSearchQuery.value.trim().toLowerCase();
      const matchMsg = log.message?.toLowerCase().includes(q);
      const matchDet = log.details?.toLowerCase().includes(q);
      const matchTbl = log.table_name?.toLowerCase().includes(q);
      if (!matchMsg && !matchDet && !matchTbl) return false;
    }
    return true;
  });
});

async function copyLogsToClipboard() {
  if (!props.item?.logs?.length) return;
  const text = props.item.logs
    .map((l) => `[${l.timestamp}] [${l.type?.toUpperCase() || 'INFO'}] ${l.message}${l.details ? ` - ${l.details}` : ''}`)
    .join('\n');
  try {
    await navigator.clipboard.writeText(text);
    copiedLogs.value = true;
    setTimeout(() => {
      copiedLogs.value = false;
    }, 2000);
  } catch (err) {
    console.error('Gagal menyalin log:', err);
  }
}
</script>

