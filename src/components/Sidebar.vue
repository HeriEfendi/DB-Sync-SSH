<template>
  <aside class="sidebar">
    <div class="sidebar-top">
      <div class="sidebar-header">
        <Database class="sidebar-brand-icon" />
        <span class="sidebar-brand-title">DB Sync Engine</span>
      </div>

      <div class="sidebar-section">
        <div class="sidebar-section-header">
          <span class="sidebar-section-title">Connections</span>
          <button
            class="sidebar-btn-add"
            title="Add New Connection Profile"
            @click="$emit('create-profile')"
          >
            <Plus :size="14" />
          </button>
        </div>

        <div class="connections-list">
          <div
            v-for="profile in profiles"
            :key="profile.id"
            class="connection-item"
            :class="{ active: currentView === 'tables' && activeProfileId === profile.id }"
            @click="$emit('select-profile', profile.id)"
          >
            <Database class="connection-icon" />
            <div class="connection-info">
              <span class="connection-name">{{ profile.name }}</span>
              <span class="connection-db-type">mysql</span>
            </div>
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
        <History :size="18" />
        <span>Sync History</span>
      </button>
      <div class="sidebar-version">v1.1.2</div>
    </div>
  </aside>
</template>

<script setup>
import { Database, Plus, History } from 'lucide-vue-next';

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

defineEmits(['select-profile', 'create-profile', 'open-history']);
</script>
