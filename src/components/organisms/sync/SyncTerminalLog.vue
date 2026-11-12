<template>
  <div class="terminal-container">
    <!-- Terminal Console Toolbar -->
    <div class="terminal-toolbar">
      <div class="terminal-tabs">
        <button
          type="button"
          class="terminal-tab"
          :class="{ active: activeFilter === 'all' }"
          @click="activeFilter = 'all'"
        >
          Semua ({{ logs.length }})
        </button>
        <button
          type="button"
          class="terminal-tab"
          :class="{ active: activeFilter === 'error' }"
          @click="activeFilter = 'error'"
        >
          <span v-if="errorCount > 0" class="tab-dot dot-error"></span>
          Error ({{ errorCount }})
        </button>
        <button
          type="button"
          class="terminal-tab"
          :class="{ active: activeFilter === 'warning' }"
          @click="activeFilter = 'warning'"
        >
          <span v-if="warningCount > 0" class="tab-dot dot-warning"></span>
          Warning ({{ warningCount }})
        </button>
        <button
          type="button"
          class="terminal-tab"
          :class="{ active: activeFilter === 'info' }"
          @click="activeFilter = 'info'"
        >
          Info ({{ infoCount }})
        </button>
      </div>

      <div class="terminal-actions">
        <div class="terminal-search-box">
          <Search :size="12" class="terminal-search-icon" />
          <input
            ref="searchInputRef"
            v-model="searchQuery"
            type="text"
            placeholder="Cari log... (Ctrl+F)"
            class="terminal-search-input"
          />
          <button
            v-if="searchQuery"
            type="button"
            class="terminal-search-clear"
            @click="searchQuery = ''"
          >
            ✕
          </button>
        </div>

        <button
          type="button"
          class="terminal-action-btn"
          title="Salin seluruh log ke clipboard"
          @click="copyAllLogs"
        >
          <Copy :size="12" />
          <span>{{ copied ? 'Tersalin!' : 'Copy' }}</span>
        </button>
        <button
          type="button"
          class="terminal-action-btn"
          title="Download log sebagai file teks (Ctrl+S)"
          @click="exportLogsToFile"
        >
          <Download :size="12" />
          <span>{{ exported ? 'Tersimpan!' : 'Export' }}</span>
        </button>
      </div>
    </div>

    <!-- Terminal Box -->
    <div class="terminal-wrapper">
      <div
        ref="terminalRef"
        class="terminal-console"
        role="log"
        aria-live="polite"
        @scroll.passive="onTerminalScroll"
      >
        <div
          v-for="(log, idx) in filteredLogs"
          :key="idx"
          class="terminal-log-row"
          :class="`log-${log.type || 'info'}`"
        >
          <div class="log-main-line" @click="toggleLog(log, idx)">
            <span class="log-time">{{ log.timestamp }}</span>
            <span class="log-text" :class="`log-msg-${log.type || 'info'}`">{{ log.message }}</span>
            <span v-if="log.details" class="log-details-toggle">
              {{ isLogExpanded(log, idx) ? '▲ Tutup' : '▼ Detail' }}
            </span>
          </div>
          <div
            v-if="log.details && isLogExpanded(log, idx)"
            class="terminal-details-block"
          >
            {{ log.details }}
          </div>
        </div>

        <div v-if="filteredLogs.length === 0" class="log-empty-state">
          <span v-if="searchQuery">Tidak ada log yang cocok dengan kata kunci pencarian.</span>
          <span v-else>Belum ada rekaman log untuk filter ini.</span>
        </div>

        <!-- Bottom spacer to ensure full visibility and prevent clipping -->
        <div class="terminal-bottom-spacer" aria-hidden="true"></div>
      </div>

      <button
        v-if="!isAutoScrollEnabled"
        type="button"
        class="terminal-scroll-bottom-btn"
        @click="scrollToBottom"
      >
        <ArrowDown :size="12" />
        <span>Gulir ke Log Terbaru</span>
      </button>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, watch, nextTick } from 'vue';
import { Search, Copy, Download, ArrowDown } from 'lucide-vue-next';

const props = defineProps({
  logs: {
    type: Array,
    default: () => [],
  },
  profileName: {
    type: String,
    default: '',
  },
});

const activeFilter = ref('all');
const searchQuery = ref('');
const copied = ref(false);
const exported = ref(false);
const terminalRef = ref(null);
const searchInputRef = ref(null);
const isAutoScrollEnabled = ref(true);

const expandedLogs = ref(new Set());
const collapsedLogs = ref(new Set());

const errorCount = computed(() => props.logs.filter((l) => l.type === 'error').length);
const warningCount = computed(() => props.logs.filter((l) => l.type === 'warning').length);
const infoCount = computed(() => props.logs.filter((l) => l.type === 'info' || !l.type).length);

const filteredLogs = computed(() => {
  let list = props.logs;
  if (activeFilter.value !== 'all') {
    list = list.filter((l) => (l.type || 'info') === activeFilter.value);
  }
  const q = searchQuery.value.trim().toLowerCase();
  if (q) {
    list = list.filter(
      (l) =>
        l.message?.toLowerCase().includes(q) ||
        l.details?.toLowerCase().includes(q) ||
        l.table_name?.toLowerCase().includes(q)
    );
  }
  return list;
});

function getLogKey(log, idx) {
  return `${idx}-${log.timestamp}-${log.message?.slice(0, 30)}`;
}

function toggleLog(log, idx) {
  const key = getLogKey(log, idx);
  const isErr = log?.type === 'error';
  const updated = new Set(isErr ? collapsedLogs.value : expandedLogs.value);
  const currentlyExpanded = isLogExpanded(log, idx);

  if (isErr) {
    if (currentlyExpanded) updated.add(key);
    else updated.delete(key);
    collapsedLogs.value = updated;
  } else {
    if (currentlyExpanded) updated.delete(key);
    else updated.add(key);
    expandedLogs.value = updated;
  }
}

function isLogExpanded(log, idx) {
  const key = getLogKey(log, idx);
  if (log?.type === 'error') return !collapsedLogs.value.has(key);
  return expandedLogs.value.has(key);
}

function onTerminalScroll() {
  if (!terminalRef.value) return;
  const { scrollTop, scrollHeight, clientHeight } = terminalRef.value;
  isAutoScrollEnabled.value = scrollHeight - scrollTop - clientHeight < 55;
}

function scrollToBottom() {
  if (terminalRef.value) {
    terminalRef.value.scrollTop = terminalRef.value.scrollHeight;
    isAutoScrollEnabled.value = true;
  }
}

watch(
  () => props.logs.length,
  async () => {
    if (isAutoScrollEnabled.value) {
      await nextTick();
      if (terminalRef.value) {
        terminalRef.value.scrollTop = terminalRef.value.scrollHeight;
      }
    }
  }
);

async function copyAllLogs() {
  if (!props.logs || props.logs.length === 0) return;
  const text = props.logs
    .map((l) => {
      let str = `[${l.timestamp}] [${(l.type || 'INFO').toUpperCase()}] ${l.message}`;
      if (l.details) str += `\n  ${l.details.split('\n').join('\n  ')}`;
      return str;
    })
    .join('\n');

  try {
    await navigator.clipboard.writeText(text);
    copied.value = true;
    setTimeout(() => { copied.value = false; }, 2000);
  } catch (err) {
    console.error('Failed to copy logs:', err);
  }
}

function exportLogsToFile() {
  if (!props.logs || props.logs.length === 0) return;
  const content = props.logs
    .map((l) => `[${l.timestamp}] [${(l.type || 'INFO').toUpperCase()}] ${l.message}${l.details ? `\n  ${l.details}` : ''}`)
    .join('\n');
  const blob = new Blob([content], { type: 'text/plain;charset=utf-8' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = `db_sync_${Date.now()}.txt`;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
  exported.value = true;
  setTimeout(() => { exported.value = false; }, 2000);
}

defineExpose({
  focusSearch: () => searchInputRef.value?.focus(),
  exportLogs: exportLogsToFile,
});
</script>

