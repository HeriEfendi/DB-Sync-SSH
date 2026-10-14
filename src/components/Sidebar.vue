<template>
  <aside class="sidebar">
    <div class="sidebar-top">
      <div class="sidebar-header">
        <div class="sidebar-brand-group">
          <Database class="sidebar-brand-icon" :size="16" />
          <span class="sidebar-brand-title">DB Sync Engine</span>
          <span class="sidebar-version-badge">v1.1.2</span>
        </div>
      </div>

      <div class="sidebar-section">
        <div class="sidebar-section-header">
          <span class="sidebar-section-title">Connections</span>
          <button
            class="sidebar-btn-add"
            title="Add New Connection Profile"
            @click="$emit('create-profile')"
          >
            <Plus :size="13" />
          </button>
        </div>

        <div class="connections-list">
          <div
            v-for="profile in profiles"
            :key="profile.id"
            class="connection-item"
            :class="{ active: currentView === 'tables' && activeProfileId === profile.id }"
            @click="$emit('select-profile', profile.id)"
            @contextmenu.prevent="openContextMenu($event, profile)"
          >
            <div class="connection-item-main">
              <Database class="connection-icon" :size="14" />
              <div class="connection-info">
                <span class="connection-name" :title="profile.name">{{ profile.name }}</span>
                <span class="connection-db-type">mysql</span>
              </div>
            </div>

            <button
              class="connection-menu-btn"
              title="Options"
              @click.stop="openContextMenu($event, profile)"
            >
              <MoreVertical :size="12" />
            </button>
          </div>
        </div>
      </div>
    </div>

    <div class="sidebar-footer">
      <button
        class="sidebar-nav-btn"
        :class="{ active: currentView === 'history' }"
        @click="$emit('open-history')"
      >
        <History :size="16" />
        <span>Sync History</span>
      </button>
    </div>

    <!-- Right-click Context Menu -->
    <div
      v-if="contextMenu.visible"
      class="sidebar-context-menu"
      :style="{ top: `${contextMenu.y}px`, left: `${contextMenu.x}px` }"
      @click.stop
    >
      <button class="context-menu-item" @click="handleEditFromMenu">
        <Pencil :size="13" />
        <span>Edit Connection</span>
      </button>
      <div class="context-menu-divider"></div>
      <button class="context-menu-item danger" @click="handlePromptDeleteFromMenu">
        <Trash2 :size="13" />
        <span>Hapus Connection</span>
      </button>
    </div>

    <!-- Confirm Delete Modal -->
    <div
      v-if="profileToDelete"
      class="confirm-modal-overlay"
      @click.self="profileToDelete = null"
    >
      <div class="confirm-modal-box">
        <div class="confirm-modal-header">
          <AlertTriangle :size="20" class="confirm-warning-icon" />
          <h3 class="confirm-title">Hapus Koneksi?</h3>
        </div>
        <p class="confirm-message">
          Apakah Anda yakin ingin menghapus koneksi <strong>"{{ profileToDelete.name }}"</strong>? Tindakan ini tidak dapat dibatalkan.
        </p>
        <div class="confirm-actions">
          <button type="button" class="btn-secondary" @click="profileToDelete = null">
            Batal
          </button>
          <button type="button" class="btn-danger-solid" @click="confirmDelete">
            Hapus
          </button>
        </div>
      </div>
    </div>
  </aside>
</template>

<script setup>
import { ref, onMounted, onBeforeUnmount } from 'vue';
import { Database, Plus, History, MoreVertical, Pencil, Trash2, AlertTriangle } from 'lucide-vue-next';

defineProps({
  profiles: {
    type: Array,
    required: true,
  },
  activeProfileId: {
    type: String,
    required: true,
  },
  currentView: {
    type: String,
    default: 'tables', // 'tables' | 'new-profile' | 'edit-profile' | 'history'
  },
});

const emit = defineEmits([
  'select-profile',
  'create-profile',
  'open-history',
  'edit-profile',
  'delete-profile',
]);

const contextMenu = ref({
  visible: false,
  x: 0,
  y: 0,
  profile: null,
});

const profileToDelete = ref(null);

function openContextMenu(event, profile) {
  const menuWidth = 160;
  const menuHeight = 85;
  const x = Math.min(event.clientX, window.innerWidth - menuWidth - 8);
  const y = Math.min(event.clientY, window.innerHeight - menuHeight - 8);

  contextMenu.value = {
    visible: true,
    x,
    y,
    profile,
  };
}

function closeContextMenu() {
  contextMenu.value.visible = false;
}

function handleEditFromMenu() {
  if (contextMenu.value.profile) {
    emit('edit-profile', contextMenu.value.profile.id);
  }
  closeContextMenu();
}

function handlePromptDeleteFromMenu() {
  if (contextMenu.value.profile) {
    profileToDelete.value = contextMenu.value.profile;
  }
  closeContextMenu();
}

function confirmDelete() {
  if (profileToDelete.value) {
    emit('delete-profile', profileToDelete.value.id);
    profileToDelete.value = null;
  }
}

function onWindowClick() {
  closeContextMenu();
}

function onKeyDown(e) {
  if (e.key === 'Escape') {
    closeContextMenu();
    profileToDelete.value = null;
  }
}

onMounted(() => {
  window.addEventListener('click', onWindowClick);
  window.addEventListener('keydown', onKeyDown);
});

onBeforeUnmount(() => {
  window.removeEventListener('click', onWindowClick);
  window.removeEventListener('keydown', onKeyDown);
});
</script>
