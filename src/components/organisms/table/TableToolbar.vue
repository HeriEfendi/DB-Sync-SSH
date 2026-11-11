<template>
  <div class="table-toolbar">
    <!-- Top Toolbar Row -->
    <div class="toolbar-top">
      <div class="toolbar-left">
        <SearchBar
          :model-value="searchQuery"
          placeholder="Search tables... (Ctrl+F)"
          @update:model-value="$emit('update:searchQuery', $event)"
          @clear="$emit('update:searchQuery', '')"
        />

        <div class="selection-actions">
          <button
            type="button"
            class="btn-chip"
            title="Pilih semua tabel hasil filter (Ctrl+A)"
            @click="$emit('select-all')"
          >
            Select All
          </button>
          <button
            type="button"
            class="btn-chip"
            title="Clear all selection"
            @click="$emit('clear-all')"
          >
            Clear
          </button>
        </div>

        <div class="filter-chips-group">
          <button
            type="button"
            class="filter-chip"
            :class="{ active: smartFilter === 'all' }"
            @click="$emit('update:smartFilter', 'all')"
          >
            All
          </button>
          <button
            type="button"
            class="filter-chip"
            :class="{ active: smartFilter === 'selected' }"
            @click="$emit('update:smartFilter', 'selected')"
          >
            Selected ({{ selectedCount }})
          </button>
        </div>
      </div>

      <div class="toolbar-right">
        <div class="template-group">
          <span class="control-label">Template:</span>
          <select
            :value="selectedTemplateId"
            class="template-select"
            @change="$emit('update:selectedTemplateId', $event.target.value)"
          >
            <option value="all">All Tables</option>
            <option
              v-for="tpl in profile.templates || []"
              :key="tpl.id"
              :value="tpl.id"
            >
              {{ tpl.name }}
            </option>
          </select>
          <button type="button" class="btn-secondary" @click="$emit('save-template')">
            Save As New
          </button>
          <button
            type="button"
            class="btn-secondary"
            :disabled="selectedTemplateId === 'all'"
            @click="$emit('update-template')"
          >
            Update Template
          </button>
        </div>
      </div>
    </div>

    <div class="toolbar-divider"></div>

    <!-- Bottom Toolbar Row -->
    <div class="toolbar-bottom">
      <div class="toolbar-left">
        <div class="button-group-wrapper">
          <span class="control-label">Mode:</span>
          <div class="pill-group">
            <button
              type="button"
              class="pill-btn"
              :class="{ active: syncMode === 'incremental' }"
              title="Sync Server (New & Update): Hanya menyalin baris data baru setelah Last ID dan mengupdate baris yang ada (REPLACE INTO)"
              @click="$emit('update:syncMode', 'incremental')"
            >
              Sync New &amp; Update
            </button>
            <button
              type="button"
              class="pill-btn warning-pill"
              :class="{ active: syncMode === 'fresh' }"
              title="Fresh Sync: Timpa bersih seluruh tabel lokal dari awal"
              @click="$emit('update:syncMode', 'fresh')"
            >
              Fresh Sync
            </button>
          </div>
        </div>

        <div class="row-limit-wrapper">
          <span class="control-label">Limit:</span>
          <select
            :value="rowLimitPreset"
            class="limit-select"
            title="Batasan jumlah baris yang akan disalin per tabel"
            @change="$emit('update:rowLimitPreset', $event.target.value === 'custom' ? 'custom' : Number($event.target.value))"
          >
            <option
              v-for="preset in ROW_LIMIT_PRESETS"
              :key="preset.value"
              :value="preset.value"
            >
              {{ preset.label }}
            </option>
          </select>
          <input
            v-if="rowLimitPreset === 'custom'"
            :value="customRowLimit"
            type="number"
            min="1"
            class="limit-custom-input"
            placeholder="Jml baris"
            @input="$emit('update:customRowLimit', Number($event.target.value))"
          />
        </div>

        <div class="row-limit-wrapper">
          <span class="control-label">Workers:</span>
          <select
            :value="concurrencyWorkers"
            class="limit-select"
            title="Jumlah worker download SSH paralel (1-4)"
            @change="$emit('update:concurrencyWorkers', Number($event.target.value))"
          >
            <option
              v-for="opt in WORKER_CONCURRENCY_OPTIONS"
              :key="opt.value"
              :value="opt.value"
            >
              {{ opt.label }}
            </option>
          </select>
        </div>

        <div class="row-limit-wrapper">
          <span class="control-label">Auto:</span>
          <select
            :value="autoSyncIntervalMinutes"
            class="limit-select"
            title="Sinkronisasi otomatis di latar belakang"
            @change="$emit('update:autoSyncIntervalMinutes', Number($event.target.value))"
          >
            <option
              v-for="opt in AUTO_SYNC_INTERVAL_OPTIONS"
              :key="opt.value"
              :value="opt.value"
            >
              {{ opt.label }}
            </option>
          </select>
          <span
            v-if="autoSyncIntervalMinutes > 0"
            class="auto-sync-countdown-badge"
            :title="`Sync otomatis berjalan setiap ${autoSyncIntervalMinutes} menit`"
          >
            ⏱️ {{ autoSyncCountdownText }}
          </span>
        </div>
      </div>

      <div class="toolbar-right">
        <button
          type="button"
          class="btn-secondary"
          :disabled="isLoadingTables || isSyncing || isComparingDiff"
          title="Bandingkan baris tabel remote vs lokal sebelum sync"
          @click="$emit('open-diff')"
        >
          <GitCompare :size="13" :class="{ 'spin-anim': isComparingDiff }" />
          <span>Compare Diff</span>
        </button>

        <button
          type="button"
          class="btn-refresh"
          :disabled="isLoadingTables"
          @click="$emit('refresh')"
        >
          <RotateCw :size="13" :class="{ 'spin-anim': isLoadingTables }" />
          <span>Refresh</span>
        </button>

        <button
          type="button"
          class="btn-primary"
          :disabled="selectedCount === 0 || isSyncing"
          :title="selectedCount > 0 ? 'Mulai sinkronisasi tabel terpilih (Shortcut: Ctrl+Enter)' : 'Pilih minimal 1 tabel untuk sync'"
          @click="$emit('start-sync')"
        >
          <Play :size="13" />
          <span>Start Sync ({{ selectedCount }})</span>
        </button>
      </div>
    </div>
  </div>
</template>

<script setup>
import { GitCompare, RotateCw, Play } from 'lucide-vue-next';
import SearchBar from '../../molecules/SearchBar.vue';
import {
  ROW_LIMIT_PRESETS,
  WORKER_CONCURRENCY_OPTIONS,
  AUTO_SYNC_INTERVAL_OPTIONS,
} from '../../../constants/syncConfig.js';

defineProps({
  searchQuery: { type: String, default: '' },
  smartFilter: { type: String, default: 'all' },
  selectedCount: { type: Number, default: 0 },
  profile: { type: Object, required: true },
  selectedTemplateId: { type: String, default: 'all' },
  syncMode: { type: String, default: 'incremental' },
  rowLimitPreset: { type: [Number, String], default: 0 },
  customRowLimit: { type: Number, default: 1000 },
  concurrencyWorkers: { type: Number, default: 2 },
  autoSyncIntervalMinutes: { type: Number, default: 0 },
  autoSyncCountdownText: { type: String, default: '00:00' },
  isLoadingTables: { type: Boolean, default: false },
  isSyncing: { type: Boolean, default: false },
  isComparingDiff: { type: Boolean, default: false },
});

defineEmits([
  'update:searchQuery',
  'update:smartFilter',
  'update:selectedTemplateId',
  'update:syncMode',
  'update:rowLimitPreset',
  'update:customRowLimit',
  'update:concurrencyWorkers',
  'update:autoSyncIntervalMinutes',
  'select-all',
  'clear-all',
  'save-template',
  'update-template',
  'open-diff',
  'refresh',
  'start-sync',
]);
</script>

