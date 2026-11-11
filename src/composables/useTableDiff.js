import { ref } from 'vue';
import { safeInvoke } from '../services/tauriHelper.js';

export function useTableDiff(props, { tableStates, setTableSelected, smartFilter, startSync }) {
  const showDiffModal = ref(false);
  const isComparingDiff = ref(false);
  const diffResults = ref([]);

  async function openDiffInspector() {
    showDiffModal.value = true;
    isComparingDiff.value = true;
    diffResults.value = [];
    try {
      const res = await safeInvoke('compare_table_counts', {
        sshConfig: props.profile.ssh,
        remoteDbConfig: props.profile.remote_db,
        localDbConfig: props.profile.local_db,
      });
      diffResults.value = Array.isArray(res) ? res : [];
    } catch (err) {
      console.error('Gagal mengambil perbandingan baris:', err);
    } finally {
      isComparingDiff.value = false;
    }
  }

  function handleSelectDiverged(names) {
    const set = new Set(names);
    Object.keys(tableStates.value).forEach((k) => setTableSelected(k, set.has(k)));
    smartFilter.value = 'selected';
  }

  function handleSyncDiverged(names) {
    handleSelectDiverged(names);
    startSync();
  }

  return {
    showDiffModal,
    isComparingDiff,
    diffResults,
    openDiffInspector,
    handleSelectDiverged,
    handleSyncDiverged,
  };
}

