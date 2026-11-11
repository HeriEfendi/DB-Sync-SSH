<template>
  <aside class="sidebar" :class="{ 'sidebar-collapsed': collapsed }">
    <!-- Drag / Toggle handle on the RIGHT edge -->
    <div
      class="sidebar-resize-handle"
      title="Seret ke kiri untuk menyembunyikan, klik untuk toggle"
      @mousedown.prevent="startDrag"
      @touchstart.prevent="startDragTouch"
      @click="onHandleClick"
    >
      <ChevronLeft :size="13" class="handle-chevron" :class="{ rotated: collapsed }" />
    </div>

    <!-- Expanded view -->
    <template v-if="!collapsed">
      <div class="sidebar-top">
        <div class="sidebar-header">
          <div class="sidebar-brand-group">
            <img class="sidebar-brand-icon" src="/favicon.svg" alt="" />
            <span class="sidebar-brand-title">DB Sync SSH</span>
            <span class="sidebar-version-badge">v{{ appVersion }}</span>
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
                type="button"
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
    </template>

    <!-- Collapsed icon-only view -->
    <template v-else>
      <div class="sidebar-collapsed-icons">
        <img class="sidebar-brand-icon" src="/favicon.svg" alt="" style="width:22px;height:22px;opacity:0.7;" />
        <div
          v-for="profile in profiles"
          :key="profile.id"
          class="collapsed-profile-dot"
          :class="{ active: currentView === 'tables' && activeProfileId === profile.id }"
          :title="profile.name"
          @click="$emit('select-profile', profile.id)"
        >
          <Database :size="14" />
        </div>
        <button
          class="collapsed-history-btn"
          :class="{ active: currentView === 'history' }"
          title="Sync History"
          @click="$emit('open-history')"
        >
          <History :size="14" />
        </button>
      </div>
    </template>

    <!-- Context Menu Component -->
    <SidebarContextMenu
      :visible="contextMenu.visible"
      :x="contextMenu.x"
      :y="contextMenu.y"
      @edit="handleEditFromMenu"
      @delete="handlePromptDeleteFromMenu"
    />

    <!-- Confirm Delete Modal Component -->
    <DeleteProfileDialog
      :profile="profileToDelete"
      @cancel="profileToDelete = null"
      @confirm="confirmDelete"
    />
  </aside>
</template>

<script setup>
import { ref, onMounted, onBeforeUnmount } from 'vue';
import { getVersion } from '@tauri-apps/api/app';
import { Database, Plus, History, MoreVertical, ChevronLeft } from 'lucide-vue-next';
import { useSidebarDrag } from '../composables/useSidebarDrag.js';
import SidebarContextMenu from './molecules/SidebarContextMenu.vue';
import DeleteProfileDialog from './molecules/DeleteProfileDialog.vue';

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
    default: 'tables',
  },
});

const emit = defineEmits([
  'select-profile',
  'create-profile',
  'open-history',
  'edit-profile',
  'delete-profile',
]);

const appVersion = ref('...');
const { collapsed, startDrag, startDragTouch, onHandleClick, cleanupDragListeners } = useSidebarDrag();

const contextMenu = ref({ visible: false, x: 0, y: 0, profile: null });
const profileToDelete = ref(null);

function openContextMenu(event, profile) {
  const menuWidth = 160;
  const menuHeight = 85;
  const x = Math.min(event.clientX, window.innerWidth - menuWidth - 8);
  const y = Math.min(event.clientY, window.innerHeight - menuHeight - 8);
  contextMenu.value = { visible: true, x, y, profile };
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

onMounted(async () => {
  appVersion.value = await getVersion();
  window.addEventListener('click', onWindowClick);
  window.addEventListener('keydown', onKeyDown);
});

onBeforeUnmount(() => {
  window.removeEventListener('click', onWindowClick);
  window.removeEventListener('keydown', onKeyDown);
  cleanupDragListeners();
});
</script>

<style scoped>
/* Modular styles imported in src/styles/sidebar.css */
</style>
