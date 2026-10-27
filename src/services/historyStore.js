/**
 * historyStore.js
 * Tracks and persists database synchronization history
 */

const STORAGE_HISTORY_KEY = 'db_sync_ssh_history';

export function getSyncHistory() {
  try {
    const raw = localStorage.getItem(STORAGE_HISTORY_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? parsed : [];
  } catch (e) {
    console.error('Error reading sync history:', e);
    return [];
  }
}

export function saveSyncHistory(history) {
  try {
    localStorage.setItem(STORAGE_HISTORY_KEY, JSON.stringify(history));
  } catch (e) {
    console.error('Error saving sync history:', e);
  }
}

export function addHistoryEntry(entry) {
  const current = getSyncHistory();
  // Sanitize entry: exclude info logs to save memory
  const sanitizedEntry = {
    ...entry,
    logs: Array.isArray(entry.logs)
      ? entry.logs.filter((l) => l.type !== 'info')
      : [],
  };

  const updated = [
    {
      id: `hist-${Date.now()}`,
      timestamp: new Date().toLocaleString(),
      ...sanitizedEntry,
    },
    ...current,
  ].slice(0, 50); // Simpan maksimal 50 riwayat terakhir untuk hemat memori

  saveSyncHistory(updated);
  return updated;
}

export function clearSyncHistory() {
  saveSyncHistory([]);
  return [];
}
