import { ref, computed } from 'vue';
import {
  getStoredProfiles,
  saveProfiles,
  getActiveProfileId,
  setActiveProfileId,
} from '../services/profileStore.js';

/**
 * Composable for managing database profiles and templates.
 * Complies with AGENTS.md: Separation of Concerns (UI vs Business Logic).
 */
export function useProfiles() {
  const profiles = ref([]);
  const activeProfileId = ref('');
  const currentView = ref('tables'); // 'tables' | 'new-profile' | 'edit-profile' | 'history'

  const activeProfile = computed(() => {
    return profiles.value.find((p) => p.id === activeProfileId.value) || profiles.value[0];
  });

  function initProfiles() {
    profiles.value = getStoredProfiles();
    const storedActiveProfileId = getActiveProfileId();
    activeProfileId.value = profiles.value.some((p) => p.id === storedActiveProfileId)
      ? storedActiveProfileId
      : profiles.value[0]?.id || '';

    if (activeProfileId.value !== storedActiveProfileId) {
      setActiveProfileId(activeProfileId.value);
    }
    currentView.value = profiles.value.length === 0 ? 'new-profile' : 'tables';
  }

  function handleSelectProfile(id) {
    activeProfileId.value = id;
    setActiveProfileId(id);
    currentView.value = 'tables';
  }

  function handleCreateProfile() {
    currentView.value = 'new-profile';
  }

  function handleEditProfile(id) {
    if (id) {
      activeProfileId.value = id;
      setActiveProfileId(id);
    }
    currentView.value = 'edit-profile';
  }

  function handleSaveProfile(savedProfile) {
    const idx = profiles.value.findIndex((p) => p.id === savedProfile.id);
    if (idx !== -1) {
      profiles.value[idx] = savedProfile;
    } else {
      profiles.value.push(savedProfile);
    }
    activeProfileId.value = savedProfile.id;
    setActiveProfileId(savedProfile.id);
    saveProfiles(profiles.value);
    currentView.value = 'tables';
  }

  function handleDeleteProfile(id) {
    profiles.value = profiles.value.filter((p) => p.id !== id);
    if (profiles.value.length === 0) {
      activeProfileId.value = '';
      setActiveProfileId('');
      handleCreateProfile();
    } else {
      activeProfileId.value = profiles.value[0].id;
      setActiveProfileId(activeProfileId.value);
      currentView.value = 'tables';
    }
    saveProfiles(profiles.value);
  }

  function handleSaveTemplate({ profileId, name, tableNames }) {
    const profile = profiles.value.find((item) => item.id === profileId);
    if (!profile) return;
    if (!profile.templates) {
      profile.templates = [];
    }
    profile.templates.push({
      id: `tpl-${Date.now()}`,
      name,
      tableNames,
    });
    saveProfiles(profiles.value);
  }

  function handleUpdateTemplate({ profileId, templateId, tableNames }) {
    const profile = profiles.value.find((item) => item.id === profileId);
    const template = profile?.templates?.find((item) => item.id === templateId);
    if (!template) return;
    template.tableNames = tableNames;
    saveProfiles(profiles.value);
  }

  return {
    profiles,
    activeProfileId,
    activeProfile,
    currentView,
    initProfiles,
    handleSelectProfile,
    handleCreateProfile,
    handleEditProfile,
    handleSaveProfile,
    handleDeleteProfile,
    handleSaveTemplate,
    handleUpdateTemplate,
  };
}

