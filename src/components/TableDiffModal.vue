<template>
  <div class="modal-overlay" @click.self="$emit('close')">
    <div class="diff-modal-content">
      <!-- Modal Header -->
      <div class="diff-modal-header">
        <div class="header-left">
          <GitCompare :size="16" class="header-icon" />
          <div>
            <h2 class="diff-title">Diff Inspector (Perbandingan Baris Tabel)</h2>
            <p class="diff-subtitle">{{ profileName }} • Remote vs MySQL Lokal</p>
          </div>
        </div>
        <button type="button" class="btn-close" title="Tutup (Esc)" @click="$emit('close')">
          ✕
        </button>
      </div>

      <!-- Loading State -->
      <div v-if="isLoading" class="diff-loading-container">
        <RotateCw :size="24" class="spin-anim loading-icon" />
        <p class="loading-text">Menghubungkan ke remote via SSH dan membaca metadata baris tabel...</p>
      </div>

      <!-- Content State -->
      <div v-else class="diff-modal-body">
        <!-- Metric Cards -->
        <div class="metrics-grid">
          <div class="metric-card">
            <span class="metric-label">Total Tabel</span>
            <strong class="metric-val">{{ diffList.length }}</strong>
          </div>
          <div class="metric-card metric-diverged">
            <span class="metric-label">Ada Perubahan</span>
            <strong class="metric-val">{{ divergedCount }}</strong>
          </div>
          <div class="metric-card metric-new">
            <span class="metric-label">Baru di Remote</span>
            <strong class="metric-val">{{ remoteOnlyCount }}</strong>
          </div>
          <div class="metric-card metric-insync">
            <span class="metric-label">Identik / In-Sync</span>
            <strong class="metric-val">{{ inSyncCount }}</strong>
          </div>
        </div>

        <!-- Filter & Search Toolbar -->
        <div class="diff-toolbar">
          <div class="diff-tabs">
            <button
              type="button"
              class="diff-tab"
              :class="{ active: activeFilter === 'all' }"
              @click="activeFilter = 'all'"
            >
              Semua ({{ diffList.length }})
            </button>
            <button
              type="button"
              class="diff-tab tab-diverged"
              :class="{ active: activeFilter === 'diverged' }"
              @click="activeFilter = 'diverged'"
            >
              Berbeda Saja ({{ divergedCount + remoteOnlyCount }})
            </button>
            <button
              type="button"
              class="diff-tab tab-insync"
              :class="{ active: activeFilter === 'in_sync' }"
              @click="activeFilter = 'in_sync'"
            >
              Identik ({{ inSyncCount }})
            </button>
          </div>

          <div class="diff-search-box">
            <Search :size="12" class="diff-search-icon" />
            <input
              v-model="searchQuery"
              type="text"
              placeholder="Cari tabel..."
              class="diff-search-input"
            />
            <button
              v-if="searchQuery"
              type="button"
              class="diff-search-clear"
              @click="searchQuery = ''"
            >
              ✕
            </button>
          </div>
        </div>

        <!-- Data Table Grid Component -->
        <DiffTableGrid :filtered-list="filteredList" />
      </div>

      <!-- Modal Footer -->
      <div class="diff-modal-footer">
        <div class="footer-left">
          <span v-if="divergedCount + remoteOnlyCount > 0" class="diverged-hint">
            💡 Ditemukan <strong>{{ divergedCount + remoteOnlyCount }}</strong> tabel yang berbeda atau baru.
          </span>
          <span v-else class="diverged-hint in-sync-hint">
            🎉 Semua tabel saat ini dalam kondisi sinkron identik!
          </span>
        </div>

        <div class="footer-right">
          <button
            v-if="divergedCount + remoteOnlyCount > 0"
            type="button"
            class="btn-secondary"
            title="Centang hanya tabel yang berbeda di tabel utama"
            @click="handleSelectDiverged"
          >
            Pilih Tabel Beda Saja
          </button>
          <button
            v-if="divergedCount + remoteOnlyCount > 0"
            type="button"
            class="btn-primary"
            style="display: inline-flex; align-items: center; gap: 5px;"
            @click="handleSyncDiverged"
          >
            <Play :size="12" />
            <span>Sync Tabel Beda ({{ divergedCount + remoteOnlyCount }})</span>
          </button>
          <button type="button" class="btn-secondary" @click="$emit('close')">
            Tutup
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, onBeforeUnmount } from 'vue';
import { GitCompare, RotateCw, Search, Play } from 'lucide-vue-next';
import DiffTableGrid from './organisms/diff/DiffTableGrid.vue';

const props = defineProps({
  diffList: {
    type: Array,
    default: () => [],
  },
  isLoading: {
    type: Boolean,
    default: false,
  },
  profileName: {
    type: String,
    default: '',
  },
});

const emit = defineEmits(['close', 'select-diverged', 'sync-diverged']);

const activeFilter = ref('all');
const searchQuery = ref('');

const divergedCount = computed(() => {
  return props.diffList.filter((item) => item.status === 'diverged').length;
});

const remoteOnlyCount = computed(() => {
  return props.diffList.filter((item) => item.status === 'remote_only').length;
});

const inSyncCount = computed(() => {
  return props.diffList.filter((item) => item.status === 'in_sync').length;
});

const filteredList = computed(() => {
  let list = props.diffList;
  if (activeFilter.value === 'diverged') {
    list = list.filter((i) => i.status === 'diverged' || i.status === 'remote_only');
  } else if (activeFilter.value === 'in_sync') {
    list = list.filter((i) => i.status === 'in_sync');
  }

  const q = searchQuery.value.trim().toLowerCase();
  if (q) {
    list = list.filter((i) => i.table_name.toLowerCase().includes(q));
  }
  return list;
});

function handleSelectDiverged() {
  const divergedNames = props.diffList
    .filter((i) => i.status === 'diverged' || i.status === 'remote_only')
    .map((i) => i.table_name);
  emit('select-diverged', divergedNames);
  emit('close');
}

function handleSyncDiverged() {
  const divergedNames = props.diffList
    .filter((i) => i.status === 'diverged' || i.status === 'remote_only')
    .map((i) => i.table_name);
  emit('sync-diverged', divergedNames);
  emit('close');
}

function onKeyDown(e) {
  if (e.key === 'Escape') {
    emit('close');
  }
}

onMounted(() => {
  window.addEventListener('keydown', onKeyDown);
});

onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKeyDown);
});
</script>

<style scoped>
/* Modular styles imported in src/styles/diff-modal.css */
</style>
