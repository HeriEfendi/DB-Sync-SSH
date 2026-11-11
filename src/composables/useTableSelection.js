import { ref, computed } from 'vue';

/**
 * Composable for table filtering and selection state management.
 * Complies with AGENTS.md: SoC, SRP (<150 lines).
 */
export function useTableSelection(tablesRef) {
  const tableStates = ref({});
  const smartFilter = ref('all'); // 'all' | 'selected'
  const searchQuery = ref('');

  function initTableStates() {
    const newStates = {};
    tablesRef.value.forEach((tbl) => {
      newStates[tbl.name] = {
        selected: true,
        sync_structure: true,
        sync_data: true,
      };
    });
    tableStates.value = newStates;
  }

  const filteredTables = computed(() => {
    let list = tablesRef.value;
    if (smartFilter.value === 'selected') {
      list = list.filter((tbl) => tableStates.value[tbl.name]?.selected);
    }
    const q = searchQuery.value.trim();
    if (!q) return list;

    if (q.startsWith('!')) {
      const negTerm = q.substring(1).trim().toLowerCase();
      if (negTerm) {
        return list.filter((tbl) => !tbl.name.toLowerCase().includes(negTerm));
      }
    }

    try {
      const reg = new RegExp(q, 'i');
      return list.filter((tbl) => reg.test(tbl.name));
    } catch {
      const low = q.toLowerCase();
      return list.filter((tbl) => tbl.name.toLowerCase().includes(low));
    }
  });

  const selectedCount = computed(() => {
    return Object.values(tableStates.value).filter((s) => s.selected).length;
  });

  const isAllSelected = computed(() => {
    if (filteredTables.value.length === 0) return false;
    return filteredTables.value.every((tbl) => tableStates.value[tbl.name]?.selected);
  });

  const isIndeterminate = computed(() => {
    const selectedInFiltered = filteredTables.value.filter(
      (tbl) => tableStates.value[tbl.name]?.selected
    ).length;
    return selectedInFiltered > 0 && selectedInFiltered < filteredTables.value.length;
  });

  function setTableSelected(tableName, selected) {
    const state = tableStates.value[tableName];
    if (state) {
      state.selected = selected;
      state.sync_data = selected;
    }
  }

  function toggleTableSelect(tableName, checked) {
    setTableSelected(tableName, checked);
  }

  function toggleSelectAllFiltered(checked) {
    filteredTables.value.forEach((tbl) => setTableSelected(tbl.name, checked));
  }

  function handleSelectAll() {
    const isFiltered = searchQuery.value.trim() !== '';
    if (isFiltered) {
      filteredTables.value.forEach((tbl) => setTableSelected(tbl.name, true));
    } else {
      Object.keys(tableStates.value).forEach((k) => setTableSelected(k, true));
    }
  }

  function handleClearAll() {
    const isFiltered = searchQuery.value.trim() !== '';
    if (isFiltered) {
      filteredTables.value.forEach((tbl) => setTableSelected(tbl.name, false));
    } else {
      Object.keys(tableStates.value).forEach((k) => setTableSelected(k, false));
    }
  }

  function toggleMode(tableName, field, checked) {
    if (tableStates.value[tableName]) {
      tableStates.value[tableName][field] = checked;
      if (checked) {
        tableStates.value[tableName].selected = true;
      }
    }
  }

  const isAllDataChecked = computed(() => {
    const selected = filteredTables.value.filter((tbl) => tableStates.value[tbl.name]?.selected);
    if (selected.length === 0) return false;
    return selected.every((tbl) => tableStates.value[tbl.name]?.sync_data);
  });

  const isDataIndeterminate = computed(() => {
    const selected = filteredTables.value.filter((tbl) => tableStates.value[tbl.name]?.selected);
    if (selected.length === 0) return false;
    const count = selected.filter((tbl) => tableStates.value[tbl.name]?.sync_data).length;
    return count > 0 && count < selected.length;
  });

  function toggleAllDataFiltered(checked) {
    const hasSelected = filteredTables.value.some((tbl) => tableStates.value[tbl.name]?.selected);
    filteredTables.value.forEach((tbl) => {
      const s = tableStates.value[tbl.name];
      if (s) {
        if (hasSelected) {
          if (s.selected) s.sync_data = checked;
        } else if (checked) {
          s.selected = true;
          s.sync_data = true;
        }
      }
    });
  }

  return {
    tableStates,
    smartFilter,
    searchQuery,
    filteredTables,
    selectedCount,
    isAllSelected,
    isIndeterminate,
    isAllDataChecked,
    isDataIndeterminate,
    initTableStates,
    setTableSelected,
    toggleTableSelect,
    toggleSelectAllFiltered,
    handleSelectAll,
    handleClearAll,
    toggleMode,
    toggleAllDataFiltered,
  };
}

