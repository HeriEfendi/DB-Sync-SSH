import { ref } from 'vue';
import {
  getSyncHistory,
  addHistoryEntry,
  clearSyncHistory,
} from '../services/historyStore.js';

/**
 * Composable for managing sync history.
 * Complies with AGENTS.md: SoC & SRP.
 */
export function useHistory() {
  const historyList = ref([]);

  function initHistory() {
    historyList.value = getSyncHistory();
  }

  function handleClearHistory() {
    historyList.value = clearSyncHistory();
  }

  function recordHistory(entry) {
    historyList.value = addHistoryEntry(entry);
  }

  return {
    historyList,
    initHistory,
    handleClearHistory,
    recordHistory,
  };
}

