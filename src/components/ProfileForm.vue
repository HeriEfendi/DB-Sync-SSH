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
      <ProfileSshFields :ssh="form.ssh" @browse-key="handleBrowseKey" />

      <!-- 3 & 4. Database Configurations -->
      <ProfileDbFields :remote-db="form.remote_db" :local-db="form.local_db" />

      <!-- Status message from tests -->
      <div
        v-if="testStatusMessage"
        class="test-status-box"
        :class="testStatusType"
      >
        {{ testStatusMessage }}
      </div>
    </div>

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
          <span>Test SSH &amp; Remote DB</span>
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
          @click="showDeleteConfirm = true"
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

    <!-- Confirm Delete Modal (Reusing DeleteProfileDialog) -->
    <DeleteProfileDialog
      :profile="showDeleteConfirm ? form : null"
      @cancel="showDeleteConfirm = false"
      @confirm="confirmDelete"
    />
  </div>
</template>

<script setup>
import { onMounted, onBeforeUnmount } from 'vue';
import { ArrowLeft, Database, CheckCircle2, Trash2 } from 'lucide-vue-next';
import { useProfileForm } from '../composables/useProfileForm.js';
import ProfileSshFields from './organisms/profile/ProfileSshFields.vue';
import ProfileDbFields from './organisms/profile/ProfileDbFields.vue';
import DeleteProfileDialog from './molecules/DeleteProfileDialog.vue';

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

const {
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
} = useProfileForm(props.initialProfile, emit);

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
  background-color: var(--primary-green-light);
  color: var(--primary-green-hover);
  border: 1px solid #a8e6c3;
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
