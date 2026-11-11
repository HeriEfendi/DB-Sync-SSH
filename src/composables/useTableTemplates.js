import { ref, watch } from 'vue';
import {
  getLastSelectedRowLimit,
  getLastSelectedTemplateId,
  setLastSelectedRowLimit,
  setLastSelectedTemplateId,
} from '../services/profileStore.js';

export function useTableTemplates(props, emit, { tableStates, handleSelectAll, setTableSelected }) {
  function getValidTemplateId(profile) {
    const storedId = getLastSelectedTemplateId(profile.id);
    if (storedId === 'all' || (profile.templates || []).some((t) => t.id === storedId)) {
      return storedId;
    }
    setLastSelectedTemplateId(profile.id, 'all');
    return 'all';
  }

  const selectedTemplateId = ref(getValidTemplateId(props.profile));
  let shouldRestoreTemplate = selectedTemplateId.value !== 'all';

  const initialRowLimit = getLastSelectedRowLimit(props.profile.id);
  const rowLimitPreset = ref(initialRowLimit.preset);
  const customRowLimit = ref(initialRowLimit.custom);

  watch(
    () => props.profile.id,
    () => {
      selectedTemplateId.value = getValidTemplateId(props.profile);
      shouldRestoreTemplate = selectedTemplateId.value !== 'all';
      const rowLimit = getLastSelectedRowLimit(props.profile.id);
      rowLimitPreset.value = rowLimit.preset;
      customRowLimit.value = rowLimit.custom;
    }
  );

  watch(
    () => [rowLimitPreset.value, customRowLimit.value],
    ([preset, custom]) => {
      setLastSelectedRowLimit(props.profile.id, preset, custom);
    }
  );

  function applyTemplate() {
    setLastSelectedTemplateId(props.profile.id, selectedTemplateId.value);
    if (selectedTemplateId.value === 'all') {
      handleSelectAll();
      return;
    }
    const tpl = (props.profile.templates || []).find((t) => t.id === selectedTemplateId.value);
    if (tpl && Array.isArray(tpl.tableNames)) {
      const set = new Set(tpl.tableNames);
      Object.keys(tableStates.value).forEach((tblName) => {
        setTableSelected(tblName, set.has(tblName));
      });
    }
  }

  function onTemplateChange(id) {
    selectedTemplateId.value = id;
    applyTemplate();
  }

  function handleSaveTemplate() {
    const name = window.prompt('Masukkan nama template untuk pilihan tabel saat ini:');
    if (!name || !name.trim()) return;
    const selectedNames = Object.entries(tableStates.value)
      .filter(([_, s]) => s.selected)
      .map(([k]) => k);
    emit('save-template', {
      profileId: props.profile.id,
      name: name.trim(),
      tableNames: selectedNames,
    });
  }

  function handleUpdateTemplate() {
    if (selectedTemplateId.value === 'all') return;
    const template = (props.profile.templates || []).find((t) => t.id === selectedTemplateId.value);
    if (!template) return;
    const selectedNames = Object.entries(tableStates.value)
      .filter(([_, s]) => s.selected)
      .map(([k]) => k);
    emit('update-template', {
      profileId: props.profile.id,
      templateId: template.id,
      tableNames: selectedNames,
    });
  }

  function checkAndRestoreTemplate() {
    if (shouldRestoreTemplate && props.tables.length > 0) {
      shouldRestoreTemplate = false;
      applyTemplate();
    }
  }

  return {
    selectedTemplateId,
    rowLimitPreset,
    customRowLimit,
    onTemplateChange,
    applyTemplate,
    handleSaveTemplate,
    handleUpdateTemplate,
    checkAndRestoreTemplate,
  };
}

