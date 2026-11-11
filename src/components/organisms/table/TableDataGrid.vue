<template>
  <div class="table-card">
    <div
      ref="containerRef"
      class="table-scroll-container"
      @scroll.passive="$emit('scroll', $event)"
    >
      <table class="data-table">
        <thead>
          <tr>
            <th class="col-select">
              <input
                type="checkbox"
                :checked="isAllSelected"
                :indeterminate.prop="isIndeterminate"
                title="Toggle Select All"
                @change="$emit('toggle-select-all', $event.target.checked)"
              />
            </th>
            <th>Table Name</th>
            <th class="col-last-id">Last ID</th>
            <th class="col-last-sync">Last Sync</th>
            <th class="col-sync-mode">
              <div class="sync-mode-header-wrap">
                <span>Sync Mode</span>
                <label class="check-label-header" title="Toggle centang 'Data' untuk semua tabel terpilih">
                  <input
                    type="checkbox"
                    :checked="isAllDataChecked"
                    :indeterminate.prop="isDataIndeterminate"
                    @change="$emit('toggle-all-data', $event.target.checked)"
                  />
                  <span>Data</span>
                </label>
              </div>
            </th>
            <th class="col-action">Action</th>
          </tr>
        </thead>
        <tbody>
          <!-- Top virtual scroll spacer row -->
          <tr
            v-if="topSpacerHeight > 0"
            class="virtual-spacer-row"
            :style="{ height: `${topSpacerHeight}px` }"
            aria-hidden="true"
          >
            <td colspan="6" style="padding: 0; border: none; height: inherit; pointer-events: none; background: transparent;"></td>
          </tr>

          <tr
            v-for="tbl in visibleTables"
            :key="tbl.name"
            :class="{
              'row-selected': tableStates[tbl.name]?.selected,
              'row-deselected': !tableStates[tbl.name]?.selected
            }"
          >
            <td class="col-select">
              <input
                type="checkbox"
                :checked="tableStates[tbl.name]?.selected"
                @change="$emit('toggle-table-select', tbl.name, $event.target.checked)"
              />
            </td>
            <td
              class="col-table-name"
              title="Click to toggle selection"
              @click="$emit('toggle-table-select', tbl.name, !tableStates[tbl.name]?.selected)"
            >
              {{ tbl.name }}
            </td>
            <td class="col-last-id">
              <span
                v-if="tableStatesMap[tbl.name]?.lastSyncedId !== undefined && tableStatesMap[tbl.name]?.lastSyncedId !== null"
                class="last-id-pill"
                :title="'Primary key: ' + (tableStatesMap[tbl.name].primaryKey || 'id')"
              >
                <span class="lbl">ID:</span>
                <strong>{{ formatLastId(tableStatesMap[tbl.name].lastSyncedId) }}</strong>
              </span>
              <span v-else class="text-dim-sync">-</span>
            </td>
            <td class="col-last-sync">
              <span
                v-if="tableStatesMap[tbl.name]?.lastSyncTime"
                class="last-time-text"
                :title="formatFullDate(
                  tableStatesMap[tbl.name].lastSyncTime,
                  tableStatesMap[tbl.name].lastSyncTimeEpochMs
                )"
              >
                {{ formatDisplayTime(
                  tableStatesMap[tbl.name].lastSyncTime,
                  tableStatesMap[tbl.name].lastSyncTimeEpochMs
                ) }}
              </span>
              <span v-else class="text-dim-sync">Belum sync</span>
            </td>
            <td class="col-sync-mode">
              <div class="sync-mode-checks">
                <label class="check-label disabled" title="Struktur tabel otomatis disinkronkan di background">
                  <input type="checkbox" :checked="true" disabled />
                  <span>Structure</span>
                </label>
                <label class="check-label" title="Centang untuk menyalin isi data tabel">
                  <input
                    type="checkbox"
                    :checked="tableStates[tbl.name]?.sync_data"
                    @change="$emit('toggle-mode', tbl.name, 'sync_data', $event.target.checked)"
                  />
                  <span>Data</span>
                </label>
              </div>
            </td>
            <td class="col-action">
              <button
                type="button"
                class="btn-quick-sync"
                :title="`Sync instan tabel '${tbl.name}' sekarang`"
                :disabled="isSyncing"
                @click.stop="$emit('quick-sync', tbl.name)"
              >
                <Zap :size="11" />
                <span>Sync</span>
              </button>
            </td>
          </tr>

          <!-- Bottom virtual scroll spacer row -->
          <tr
            v-if="bottomSpacerHeight > 0"
            class="virtual-spacer-row"
            :style="{ height: `${bottomSpacerHeight}px` }"
            aria-hidden="true"
          >
            <td colspan="6" style="padding: 0; border: none; height: inherit; pointer-events: none; background: transparent;"></td>
          </tr>

          <tr v-if="filteredTables.length === 0">
            <td colspan="6" class="empty-state-cell">
              <span v-if="isLoadingTables">Memuat daftar tabel remote...</span>
              <span v-else>Tidak ada tabel yang cocok dengan pencarian atau koneksi belum dimuat.</span>
            </td>
          </tr>
        </tbody>
      </table>
    </div>

    <!-- Table Footer Status -->
    <div class="table-footer-status">
      <span>Menampilkan <strong>{{ filteredTables.length }}</strong> tabel</span>
      <span>Terpilih: <strong style="color: var(--primary-green)">{{ selectedCount }}</strong> dari {{ totalTablesCount }} tabel</span>
    </div>
  </div>
</template>

<script setup>
import { ref } from 'vue';
import { Zap } from 'lucide-vue-next';
import { formatLastId, formatDisplayTime, formatFullDate } from '../../../utils/formatters.js';

defineProps({
  filteredTables: { type: Array, required: true },
  visibleTables: { type: Array, required: true },
  topSpacerHeight: { type: Number, default: 0 },
  bottomSpacerHeight: { type: Number, default: 0 },
  tableStates: { type: Object, required: true },
  tableStatesMap: { type: Object, default: () => ({}) },
  isAllSelected: { type: Boolean, default: false },
  isIndeterminate: { type: Boolean, default: false },
  isAllDataChecked: { type: Boolean, default: false },
  isDataIndeterminate: { type: Boolean, default: false },
  isLoadingTables: { type: Boolean, default: false },
  isSyncing: { type: Boolean, default: false },
  totalTablesCount: { type: Number, default: 0 },
  selectedCount: { type: Number, default: 0 },
});

defineEmits([
  'toggle-select-all',
  'toggle-all-data',
  'toggle-table-select',
  'toggle-mode',
  'quick-sync',
  'scroll',
]);

const containerRef = ref(null);

defineExpose({
  containerRef,
});
</script>

