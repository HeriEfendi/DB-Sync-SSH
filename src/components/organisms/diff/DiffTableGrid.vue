<template>
  <div class="diff-table-container">
    <table class="diff-table">
      <thead>
        <tr>
          <th>Nama Tabel</th>
          <th style="text-align: right; width: 110px;">Remote Rows</th>
          <th style="text-align: right; width: 110px;">Lokal Rows</th>
          <th style="text-align: right; width: 100px;">Selisih</th>
          <th style="text-align: center; width: 130px;">Status</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="item in filteredList" :key="item.table_name">
          <td class="col-diff-name font-mono">{{ item.table_name }}</td>
          <td class="col-diff-num font-mono">
            {{ item.remote_rows >= 0 ? formatNumber(item.remote_rows) : '-' }}
          </td>
          <td class="col-diff-num font-mono">
            {{ item.local_rows >= 0 ? formatNumber(item.local_rows) : 'Belum Ada' }}
          </td>
          <td class="col-diff-diff font-mono">
            <span
              v-if="item.status === 'diverged'"
              :class="item.row_diff > 0 ? 'diff-plus' : 'diff-minus'"
            >
              {{ item.row_diff > 0 ? '+' : '' }}{{ formatNumber(item.row_diff) }}
            </span>
            <span v-else-if="item.status === 'remote_only'" class="diff-plus">
              +{{ formatNumber(item.remote_rows) }}
            </span>
            <span v-else-if="item.status === 'in_sync'" class="diff-zero">
              0
            </span>
            <span v-else class="diff-zero">-</span>
          </td>
          <td style="text-align: center;">
            <span
              class="diff-badge"
              :class="{
                'badge-diverged': item.status === 'diverged',
                'badge-insync': item.status === 'in_sync',
                'badge-new': item.status === 'remote_only',
                'badge-local': item.status === 'local_only',
              }"
            >
              <span v-if="item.status === 'diverged'">⚡ Beda</span>
              <span v-else-if="item.status === 'in_sync'">✓ Identik</span>
              <span v-else-if="item.status === 'remote_only'">🆕 Remote Baru</span>
              <span v-else>{{ item.status }}</span>
            </span>
          </td>
        </tr>
        <tr v-if="filteredList.length === 0">
          <td colspan="5" class="diff-empty-cell">
            Tidak ada tabel yang cocok dengan filter.
          </td>
        </tr>
      </tbody>
    </table>
  </div>
</template>

<script setup>
import { formatNumber } from '../../../utils/formatters.js';

defineProps({
  filteredList: {
    type: Array,
    default: () => [],
  },
});
</script>

