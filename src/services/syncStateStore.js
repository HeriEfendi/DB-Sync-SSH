/**
 * syncStateStore.js
 * 
 * Tracks and persists last sync state per profile and table.
 * Key format: db_sync_ssh_tstate_{profileId}_{tableName}
 */

const STORE_PREFIX = 'db_sync_ssh_tstate_';

function sanitizeSegment(s) {
  return (s || 'unknown').replace(/[^a-zA-Z0-9_.-]/g, '_');
}

export function buildKey(profileId, tableName) {
  const pId = sanitizeSegment(profileId);
  const tbl = sanitizeSegment(tableName);
  return `${STORE_PREFIX}${pId}_${tbl}`;
}

export function getTableState(profileId, tableName) {
  try {
    const key = buildKey(profileId, tableName);
    const raw = localStorage.getItem(key);
    if (!raw) return null;
    return JSON.parse(raw);
  } catch {
    return null;
  }
}

export function saveTableState(profileId, tableName, state) {
  try {
    const key = buildKey(profileId, tableName);
    const existing = getTableState(profileId, tableName) || {};
    const merged = {
      ...existing,
      ...state,
      _profileId: profileId,
      _table: tableName,
      updatedAt: new Date().toISOString(),
    };
    localStorage.setItem(key, JSON.stringify(merged));
    return merged;
  } catch (e) {
    console.warn('[syncStateStore] Failed to save state:', e);
    return null;
  }
}

export function saveMultipleTableStates(profileId, stateMap) {
  try {
    for (const [tableName, state] of Object.entries(stateMap)) {
      saveTableState(profileId, tableName, state);
    }
  } catch (e) {
    console.warn('[syncStateStore] Failed to save multiple states:', e);
  }
}

export function clearTableState(profileId, tableName) {
  try {
    const key = buildKey(profileId, tableName);
    localStorage.removeItem(key);
  } catch (e) {
    console.warn('[syncStateStore] Failed to clear state:', e);
  }
}

export function clearAllTableStates(profileId) {
  try {
    const pId = sanitizeSegment(profileId);
    const targetPrefix = `${STORE_PREFIX}${pId}_`;
    const keysToRemove = [];
    for (let i = 0; i < localStorage.length; i++) {
      const key = localStorage.key(i);
      if (key && (profileId ? key.startsWith(targetPrefix) : key.startsWith(STORE_PREFIX))) {
        keysToRemove.push(key);
      }
    }
    keysToRemove.forEach((k) => localStorage.removeItem(k));
  } catch (e) {
    console.warn('[syncStateStore] Failed to clear all states:', e);
  }
}

export function getAllTableStates(profileId) {
  const map = {};
  try {
    const pId = sanitizeSegment(profileId);
    const targetPrefix = `${STORE_PREFIX}${pId}_`;
    for (let i = 0; i < localStorage.length; i++) {
      const key = localStorage.key(i);
      if (!key || !key.startsWith(targetPrefix)) continue;

      const raw = localStorage.getItem(key);
      if (!raw) continue;

      try {
        const state = JSON.parse(raw);
        const tbl = state._table || key.replace(targetPrefix, '');
        map[tbl] = state;
      } catch {
        // Skip
      }
    }
  } catch (e) {
    console.warn('[syncStateStore] Failed to read all states:', e);
  }
  return map;
}
