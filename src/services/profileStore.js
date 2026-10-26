/**
 * profileStore.js
 * Manages connection profiles and table selection templates
 */

const STORAGE_PROFILES_KEY = 'db_sync_ssh_profiles';
const STORAGE_ACTIVE_ID_KEY = 'db_sync_ssh_active_profile_id';
const STORAGE_TEMPLATE_SELECTION_PREFIX = 'db_sync_ssh_template_selection_';
const STORAGE_ROW_LIMIT_PREFIX = 'db_sync_ssh_row_limit_';
const ROW_LIMIT_PRESETS = [0, 1000, 10000, 50000, 100000, 500000, 1000000, 2000000, 5000000];

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

export function getLastSelectedTemplateId(profileId) {
  try {
    return localStorage.getItem(`${STORAGE_TEMPLATE_SELECTION_PREFIX}${profileId}`) || 'all';
  } catch (e) {
    console.error('Error loading last selected template ID:', e);
    return 'all';
  }
}

export function setLastSelectedTemplateId(profileId, templateId) {
  try {
    localStorage.setItem(`${STORAGE_TEMPLATE_SELECTION_PREFIX}${profileId}`, templateId);
  } catch (e) {
    console.error('Error saving last selected template ID:', e);
  }
}

export function getLastSelectedRowLimit(profileId) {
  try {
    const raw = localStorage.getItem(`${STORAGE_ROW_LIMIT_PREFIX}${profileId}`);
    if (!raw) return { preset: 0, custom: 10000 };

    const stored = JSON.parse(raw);
    const validCustom =
      Number.isSafeInteger(stored.custom) && stored.custom > 0 ? stored.custom : 10000;
    if (stored.preset === 'custom') {
      return { preset: 'custom', custom: validCustom };
    }
    if (ROW_LIMIT_PRESETS.includes(stored.preset)) {
      return { preset: stored.preset, custom: validCustom };
    }
    return { preset: 0, custom: validCustom };
  } catch (e) {
    console.error('Error loading last selected row limit:', e);
    return { preset: 0, custom: 10000 };
  }
}

export function setLastSelectedRowLimit(profileId, preset, custom) {
  try {
    localStorage.setItem(
      `${STORAGE_ROW_LIMIT_PREFIX}${profileId}`,
      JSON.stringify({ preset, custom })
    );
  } catch (e) {
    console.error('Error saving last selected row limit:', e);
  }
}
