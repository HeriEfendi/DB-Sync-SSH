<template>
  <aside class="sidebar" :class="{ 'sidebar-collapsed': collapsed }">
    <!-- Drag / Toggle handle on the RIGHT edge -->
    <div
      class="sidebar-resize-handle"
      @mousedown.prevent="startDrag"
      @touchstart.prevent="startDragTouch"
      @click="onHandleClick"
      title="Seret ke kiri untuk menyembunyikan, klik untuk toggle"
    >
      <ChevronLeft :size="13" class="handle-chevron" :class="{ 'rotated': collapsed }" />
    </div>

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

    <!-- Collapsed icon-only state -->
    <template v-if="collapsed">
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
import { getVersion } from '@tauri-apps/api/app';
import { Database, Plus, History, MoreVertical, Pencil, Trash2, AlertTriangle, ChevronLeft } from 'lucide-vue-next';

const appVersion = ref('...');

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

// ── Collapse / drag state ──────────────────────────────────────────────────
const collapsed = ref(false);
const COLLAPSE_THRESHOLD = 60; // px dragged to the left to trigger collapse
let dragStartX = 0;
let dragActive = false;
let movedEnough = false;

function startDrag(e) {
  dragActive = true;
  movedEnough = false;
  dragStartX = e.clientX;
  window.addEventListener('mousemove', onDragMove);
  window.addEventListener('mouseup', onDragEnd);
}

function startDragTouch(e) {
  dragActive = true;
  movedEnough = false;
  dragStartX = e.touches[0].clientX;
  window.addEventListener('touchmove', onDragMoveTouch, { passive: false });
  window.addEventListener('touchend', onDragEndTouch);
}

function onDragMove(e) {
  if (!dragActive) return;
  const delta = dragStartX - e.clientX; // positive = dragged left
  if (delta > 8) movedEnough = true;
  if (delta >= COLLAPSE_THRESHOLD && !collapsed.value) {
    collapsed.value = true;
    endDrag();
  }
}

function onDragMoveTouch(e) {
  if (!dragActive) return;
  e.preventDefault();
  const delta = dragStartX - e.touches[0].clientX;
  if (delta > 8) movedEnough = true;
  if (delta >= COLLAPSE_THRESHOLD && !collapsed.value) {
    collapsed.value = true;
    endDragTouch();
  }
}

function onDragEnd() {
  endDrag();
}

function onDragEndTouch() {
  endDragTouch();
}

function endDrag() {
  dragActive = false;
  window.removeEventListener('mousemove', onDragMove);
  window.removeEventListener('mouseup', onDragEnd);
}

function endDragTouch() {
  dragActive = false;
  window.removeEventListener('touchmove', onDragMoveTouch);
  window.removeEventListener('touchend', onDragEndTouch);
}

function onHandleClick() {
  // Only toggle if it was a click (not a drag)
  if (!movedEnough) {
    collapsed.value = !collapsed.value;
  }
  movedEnough = false;
}
// ──────────────────────────────────────────────────────────────────────────

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

onMounted(async () => {
  appVersion.value = await getVersion();
  window.addEventListener('click', onWindowClick);
  window.addEventListener('keydown', onKeyDown);
});

onBeforeUnmount(() => {
  window.removeEventListener('click', onWindowClick);
  window.removeEventListener('keydown', onKeyDown);
  window.removeEventListener('mousemove', onDragMove);
  window.removeEventListener('mouseup', onDragEnd);
  window.removeEventListener('touchmove', onDragMoveTouch);
  window.removeEventListener('touchend', onDragEndTouch);
});
</script>
