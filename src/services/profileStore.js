/**
 * profileStore.js
 * Manages connection profiles and table selection templates
 */

const STORAGE_PROFILES_KEY = 'db_sync_ssh_profiles';
const STORAGE_ACTIVE_ID_KEY = 'db_sync_ssh_active_profile_id';

export function getStoredProfiles() {
  try {
    const raw = localStorage.getItem(STORAGE_PROFILES_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? parsed : [];
  } catch (e) {
    console.error('Error loading profiles from localStorage:', e);
    return [];
  }
}

export function saveProfiles(profiles) {
  try {
    localStorage.setItem(STORAGE_PROFILES_KEY, JSON.stringify(profiles));
  } catch (e) {
    console.error('Error saving profiles to localStorage:', e);
  }
}

export function getActiveProfileId() {
  try {
    return localStorage.getItem(STORAGE_ACTIVE_ID_KEY) || '';
  } catch (e) {
    console.error('Error loading active profile ID:', e);
    return '';
  }
}

export function setActiveProfileId(id) {
  try {
    localStorage.setItem(STORAGE_ACTIVE_ID_KEY, id);
  } catch (e) {
    console.error('Error saving active profile ID:', e);
  }
}
