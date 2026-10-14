/**
 * historyStore.js
 * Tracks and persists database synchronization history
 */

const STORAGE_HISTORY_KEY = 'db_sync_ssh_history';

const INITIAL_HISTORY = [
  {
    id: 'hist-1',
    profile_name: 'staging_olshoperp2',
    status: 'Completed',
    tables_count: 675,
    rows_copied: 582012,
    duration_text: '2m 19s',
    timestamp: '4/8/2026, 15.58.54',
  },
  {
    id: 'hist-2',
    profile_name: 'staging_olshoperp2',
    status: 'Completed',
    tables_count: 96,
    rows_copied: 3222413,
    duration_text: '2m 46s',
    timestamp: '4/8/2026, 10.18.06',
  },
  {
    id: 'hist-3',
    profile_name: 'staging_olshoperp2',
    status: 'Completed',
    tables_count: 147,
    rows_copied: 6438546,
    duration_text: '7m 32s',
    timestamp: '4/8/2026, 09.34.07',
  },
  {
    id: 'hist-4',
    profile_name: 'staging_olshoperp2',
    status: 'Aborted',
    tables_count: 147,
    rows_copied: 3024602,
    duration_text: '11m 18s',
    timestamp: '4/8/2026, 09.26.29',
  },
];

export function getSyncHistory() {
  try {
    const raw = localStorage.getItem(STORAGE_HISTORY_KEY);
    if (!raw) {
      saveSyncHistory(INITIAL_HISTORY);
      return INITIAL_HISTORY;
    }
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? parsed : INITIAL_HISTORY;
  } catch (e) {
    console.error('Error reading sync history:', e);
    return INITIAL_HISTORY;
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
  const updated = [
    {
      id: `hist-${Date.now()}`,
      timestamp: new Date().toLocaleString(),
      ...entry,
    },
    ...current,
  ];
  saveSyncHistory(updated);
  return updated;
}

export function clearSyncHistory() {
  saveSyncHistory([]);
  return [];
}
