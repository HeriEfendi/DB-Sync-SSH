<template>
  <div class="search-input-wrapper">
    <Search class="search-icon" :size="iconSize" />
    <input
      ref="inputRef"
      :value="modelValue"
      type="text"
      class="search-input"
      :placeholder="placeholder"
      @input="$emit('update:modelValue', $event.target.value)"
      @keydown.esc="clearSearch"
    />
    <button
      v-if="modelValue"
      type="button"
      class="search-clear-btn"
      title="Clear search (Esc)"
      @click="clearSearch"
    >
      ✕
    </button>
  </div>
</template>

<script setup>
import { ref } from 'vue';
import { Search } from 'lucide-vue-next';

defineProps({
  modelValue: {
    type: String,
    default: '',
  },
  placeholder: {
    type: String,
    default: 'Search...',
  },
  iconSize: {
    type: Number,
    default: 13,
  },
});

const emit = defineEmits(['update:modelValue', 'clear']);
const inputRef = ref(null);

function clearSearch() {
  emit('update:modelValue', '');
  emit('clear');
  inputRef.value?.focus();
}

defineExpose({
  focus: () => inputRef.value?.focus(),
});
</script>

