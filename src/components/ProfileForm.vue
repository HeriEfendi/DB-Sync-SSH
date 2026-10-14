<template>
  <div class="form-card-container">
    <header class="form-header">
      <button class="back-btn" title="Kembali" @click="$emit('cancel')">
        <ArrowLeft :size="14" />
      </button>
      <h1 class="main-title">
        {{ isEditing ? 'Edit Connection Profile' : 'New Connection Profile' }}
      </h1>
    </header>

    <!-- Scrollable Form Body -->
    <div class="form-scrollable-body">
      <!-- 1. General Section -->
    <div class="form-section">
      <div class="section-title-row">
        <Database :size="14" />
        <span>General</span>
      </div>
      <div class="form-grid">
        <div class="form-group">
          <label class="form-label">Profile Name *</label>
          <input
            v-model="form.name"
            type="text"
            class="form-input"
            placeholder="e.g. OLSHOP ERP Staging"
            required
          />
        </div>
        <div class="form-group">
          <label class="form-label">Database Type *</label>
          <select v-model="form.db_type" class="form-select">
            <option value="MySQL / MariaDB">MySQL / MariaDB</option>
          </select>
        </div>
      </div>
    </div>

    <!-- 2. SSH Configuration -->
    <div class="form-section">
      <div class="section-title-row">
        <Shield :size="14" />
        <span>SSH Configuration</span>
      </div>
      <div class="form-grid">
        <div class="form-group">
          <label class="form-label">SSH Host *</label>
          <input
            v-model="form.ssh.host"
            type="text"
            class="form-input"
            placeholder="123.45.67.89 or example.com"
            required
          />
        </div>
        <div class="form-group">
          <label class="form-label">SSH Port *</label>
          <input
            v-model.number="form.ssh.port"
            type="number"
            class="form-input"
            placeholder="22"
            required
          />
        </div>
        <div class="form-group">
          <label class="form-label">SSH Username *</label>
          <input
            v-model="form.ssh.username"
            type="text"
            class="form-input"
            placeholder="ubuntu, root, etc."
            required
          />
        </div>
        <div class="form-group">
          <label class="form-label">Auth Method *</label>
          <select v-model="form.ssh.auth_method" class="form-select">
            <option value="key_no_pass">SSH Key (No Password)</option>
            <option value="key_with_passphrase">SSH Key with Passphrase</option>
            <option value="password">Password</option>
            <option value="agent">SSH Agent / Default Keys (~/.ssh)</option>
          </select>
        </div>

        <!-- Key Path if Key Auth -->
        <div
          v-if="form.ssh.auth_method === 'key_no_pass' || form.ssh.auth_method === 'key_with_passphrase'"
          class="form-group full-width"
        >
          <label class="form-label">Private Key Path *</label>
          <div class="input-browse-group">
            <input
              v-model="form.ssh.private_key_path"
              type="text"
              class="form-input"
              placeholder="/home/user/.ssh/id_rsa"
            />
            <button type="button" class="btn-secondary" @click="handleBrowseKey">
              Browse
            </button>
          </div>
        </div>

        <!-- Passphrase if Key with Passphrase -->
        <div
          v-if="form.ssh.auth_method === 'key_with_passphrase'"
          class="form-group full-width"
        >
          <label class="form-label">Key Passphrase</label>
          <input
            v-model="form.ssh.passphrase"
            type="password"
            class="form-input"
            placeholder="Passphrase private key"
          />
        </div>

        <!-- SSH Password if Password Auth -->
        <div
          v-if="form.ssh.auth_method === 'password'"
          class="form-group full-width"
        >
          <label class="form-label">SSH Password *</label>
          <input
            v-model="form.ssh.password"
            type="password"
            class="form-input"
            placeholder="Password user SSH"
          />
        </div>
      </div>
    </div>

    <!-- 3. Remote Database Configuration -->
    <div class="form-section">
      <div class="section-title-row">
        <Server :size="14" />
        <span>Remote Database Configuration</span>
      </div>
      <div class="form-grid">
        <div class="form-group">
          <label class="form-label">Database Host (relative to server) *</label>
          <input
            v-model="form.remote_db.host"
            type="text"
            class="form-input"
            placeholder="localhost"
            required
          />
        </div>
        <div class="form-group">
          <label class="form-label">Database Port *</label>
          <input
            v-model.number="form.remote_db.port"
            type="number"
            class="form-input"
            placeholder="3306"
            required
          />
        </div>
        <div class="form-group">
          <label class="form-label">Database User *</label>
          <input
            v-model="form.remote_db.username"
            type="text"
            class="form-input"
            placeholder="db_user"
            required
          />
        </div>
        <div class="form-group">
          <label class="form-label">Database Password</label>
          <input
            v-model="form.remote_db.password"
            type="password"
            class="form-input"
            placeholder="db_password"
          />
        </div>
        <div class="form-group full-width">
          <label class="form-label">Database Name *</label>
          <input
            v-model="form.remote_db.database"
            type="text"
            class="form-input"
            placeholder="olshoperp_staging"
            required
          />
        </div>
      </div>
    </div>

    <!-- 4. Local Database Configuration -->
    <div class="form-section">
      <div class="section-title-row">
        <HardDrive :size="14" />
        <span>Local Database Configuration</span>
      </div>
      <div class="form-grid">
        <div class="form-group">
          <label class="form-label">Local Host *</label>
          <input
            v-model="form.local_db.host"
            type="text"
            class="form-input"
            placeholder="127.0.0.1"
            required
          />
        </div>
        <div class="form-group">
          <label class="form-label">Local Port *</label>
          <input
            v-model.number="form.local_db.port"
            type="number"
            class="form-input"
            placeholder="3306"
            required
          />
        </div>
        <div class="form-group">
          <label class="form-label">Local User *</label>
          <input
            v-model="form.local_db.username"
            type="text"
            class="form-input"
            placeholder="root"
            required
          />
        </div>
        <div class="form-group">
          <label class="form-label">Local Password</label>
          <input
            v-model="form.local_db.password"
            type="password"
            class="form-input"
            placeholder=""
          />
        </div>
        <div class="form-group full-width">
          <label class="form-label">Local Database Name *</label>
          <input
            v-model="form.local_db.database"
            type="text"
            class="form-input"
            placeholder="staging_olshoperp2"
            required
          />
        </div>

        <div class="form-group full-width" style="margin-top: 0.5rem">
          <label class="check-label">
            <input v-model="form.local_db.use_docker" type="checkbox" />
            <span>Use Docker Container for MySQL Local</span>
          </label>
        </div>

        <div v-if="form.local_db.use_docker" class="form-group full-width">
          <label class="form-label">Docker Container Name *</label>
          <input
            v-model="form.local_db.docker_container"
            type="text"
            class="form-input"
            placeholder="e.g. mysql_staging or my-mariadb"
          />
        </div>
      </div>
    </div>

    <!-- Status message from tests -->
    <div
      v-if="testStatusMessage"
      class="test-status-box"
      :class="testStatusType"
    >
      {{ testStatusMessage }}
    </div>
    </div> <!-- end .form-scrollable-body -->

    <!-- Static Action Footer on Bottom -->
    <footer class="form-actions-footer">
      <div class="footer-left">
        <button
          type="button"
          class="btn-secondary"
          :disabled="isTesting"
          @click="runTestSshAndRemote"
        >
          <CheckCircle2 :size="14" />
          <span>Test SSH & Remote DB</span>
        </button>
        <button
          type="button"
          class="btn-secondary"
          :disabled="isTesting"
          @click="runTestLocal"
        >
          <CheckCircle2 :size="14" />
          <span>Test Local DB</span>
        </button>
      </div>

      <div class="footer-right">
        <button
          v-if="isEditing"
          type="button"
          class="btn-danger"
          @click="handleDelete"
        >
          <Trash2 :size="13" />
          <span>Delete</span>
        </button>
        <button type="button" class="btn-secondary" @click="$emit('cancel')">
          Cancel
        </button>
        <button type="button" class="btn-primary" @click="handleSave">
          Save Profile
        </button>
      </div>
    </footer>

    <!-- Confirm Delete Modal -->
    <div
      v-if="showDeleteConfirm"
      class="confirm-modal-overlay"
      @click.self="showDeleteConfirm = false"
    >
      <div class="confirm-modal-box">
        <div class="confirm-modal-header">
          <AlertTriangle :size="20" class="confirm-warning-icon" />
          <h3 class="confirm-title">Hapus Koneksi?</h3>
        </div>
        <p class="confirm-message">
          Apakah Anda yakin ingin menghapus koneksi <strong>"{{ form.name }}"</strong>? Tindakan ini tidak dapat dibatalkan.
        </p>
        <div class="confirm-actions">
          <button type="button" class="btn-secondary" @click="showDeleteConfirm = false">
            Batal
          </button>
          <button type="button" class="btn-danger-solid" @click="confirmDelete">
            Hapus
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, onMounted, onBeforeUnmount } from 'vue';
import {
  ArrowLeft,
  Database,
  Shield,
  Server,
  HardDrive,
  CheckCircle2,
  Trash2,
  AlertTriangle,
} from 'lucide-vue-next';
import { safeInvoke } from '../services/tauriHelper.js';

const props = defineProps({
  initialProfile: {
    type: Object,
    default: null,
  },
  isEditing: {
    type: Boolean,
    default: false,
  },
});

const emit = defineEmits(['save', 'cancel', 'delete']);

const form = ref(
  props.initialProfile
    ? JSON.parse(JSON.stringify(props.initialProfile))
    : {
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
      }
);

const isTesting = ref(false);
const testStatusMessage = ref('');
const testStatusType = ref('info'); // 'success' | 'error' | 'info'

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

const showDeleteConfirm = ref(false);

function handleDelete() {
  showDeleteConfirm.value = true;
}

function confirmDelete() {
  showDeleteConfirm.value = false;
  emit('delete', form.value.id);
}

function onKeyDown(e) {
  if (e.key === 'Escape' && showDeleteConfirm.value) {
    showDeleteConfirm.value = false;
  }
}

onMounted(() => {
  window.addEventListener('keydown', onKeyDown);
});

onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKeyDown);
});
</script>

<style scoped>
.test-status-box {
  padding: 0.875rem 1.25rem;
  border-radius: var(--radius-md);
  font-size: 0.875rem;
  font-weight: 500;
  margin-top: 1rem;
}
.test-status-box.info {
  background-color: #eff6ff;
  color: #1e40af;
  border: 1px solid #bfdbfe;
}
.test-status-box.success {
  background-color: var(--success-green-bg);
  color: #065f46;
  border: 1px solid #a7f3d0;
}
.test-status-box.error {
  background-color: var(--danger-red-bg);
  color: #991b1b;
  border: 1px solid #fecaca;
}
</style>
