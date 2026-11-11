import { ref } from 'vue';
import { safeInvoke } from '../services/tauriHelper.js';

export function createDefaultProfile() {
  return {
    id: `prof-${Date.now()}`,
    name: '',
    db_type: 'MySQL / MariaDB',
    ssh: {
      host: '',
      port: 22,
      username: '',
      auth_method: 'key_no_pass',
      private_key_path: '/home/lenovo/.ssh/id_rsa',
      passphrase: '',
      password: '',
    },
    remote_db: {
      host: 'localhost',
      port: 3306,
      username: '',
      password: '',
      database: '',
    },
    local_db: {
      host: '127.0.0.1',
      port: 3306,
      username: 'root',
      password: '',
      database: '',
      use_docker: false,
      docker_container: '',
    },
    templates: [],
  };
}

/**
 * Composable for connection profile form handling, validation, and connectivity tests.
 * Complies with AGENTS.md: SoC, SRP (<150 lines).
 */
export function useProfileForm(initialProfile, emit) {
  const form = ref(
    initialProfile
      ? JSON.parse(JSON.stringify(initialProfile))
      : createDefaultProfile()
  );

  const isTesting = ref(false);
  const testStatusMessage = ref('');
  const testStatusType = ref('info'); // 'success' | 'error' | 'info'
  const showDeleteConfirm = ref(false);

  async function handleBrowseKey() {
    try {
      const selected = await safeInvoke('pick_private_key_file');
      if (selected) {
        form.value.ssh.private_key_path = selected;
      }
    } catch (err) {
      console.error('Gagal memilih file:', err);
    }
  }

  async function runTestSshAndRemote() {
    isTesting.value = true;
    testStatusMessage.value = 'Menguji koneksi SSH dan MySQL remote...';
    testStatusType.value = 'info';

    try {
      const res = await safeInvoke('test_ssh_and_remote_db', {
        sshConfig: form.value.ssh,
        remoteDbConfig: form.value.remote_db,
      });
      testStatusMessage.value = `✅ ${res}`;
      testStatusType.value = 'success';
    } catch (err) {
      testStatusMessage.value = `❌ Gagal: ${err}`;
      testStatusType.value = 'error';
    } finally {
      isTesting.value = false;
    }
  }

  async function runTestLocal() {
    isTesting.value = true;
    testStatusMessage.value = 'Menguji koneksi MySQL lokal...';
    testStatusType.value = 'info';

    try {
      const res = await safeInvoke('test_local_connection', {
        config: form.value.local_db,
      });
      testStatusMessage.value = `✅ MySQL Lokal OK (${res})`;
      testStatusType.value = 'success';
    } catch (err) {
      testStatusMessage.value = `❌ Gagal: ${err}`;
      testStatusType.value = 'error';
    } finally {
      isTesting.value = false;
    }
  }

  function handleSave() {
    if (!form.value.name.trim()) {
      alert('Nama profile harus diisi');
      return;
    }
    if (!form.value.ssh.host.trim()) {
      alert('SSH Host harus diisi');
      return;
    }
    if (!form.value.remote_db.database.trim()) {
      alert('Remote database name harus diisi');
      return;
    }
    if (!form.value.local_db.database.trim()) {
      form.value.local_db.database = form.value.remote_db.database;
    }

    emit('save', JSON.parse(JSON.stringify(form.value)));
  }

  function confirmDelete() {
    showDeleteConfirm.value = false;
    emit('delete', form.value.id);
  }

  return {
    form,
    isTesting,
    testStatusMessage,
    testStatusType,
    showDeleteConfirm,
    handleBrowseKey,
    runTestSshAndRemote,
    runTestLocal,
    handleSave,
    confirmDelete,
  };
}

