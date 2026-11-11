<template>
  <div
    v-if="visible"
    class="sidebar-context-menu"
    :style="{ top: `${y}px`, left: `${x}px` }"
    @click.stop
  >
    <button type="button" class="context-menu-item" @click="$emit('edit')">
      <Pencil :size="13" />
      <span>Edit Connection</span>
    </button>
    <div class="context-menu-divider"></div>
    <button type="button" class="context-menu-item danger" @click="$emit('delete')">
      <Trash2 :size="13" />
      <span>Hapus Connection</span>
    </button>
  </div>
</template>

<script setup>
import { Pencil, Trash2 } from 'lucide-vue-next';

defineProps({
  visible: {
    type: Boolean,
    default: false,
  },
  x: {
    type: Number,
    default: 0,
  },
  y: {
    type: Number,
    default: 0,
  },
});

defineEmits(['edit', 'delete']);
</script>

