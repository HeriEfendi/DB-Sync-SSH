<template>
  <button
    :type="type"
    :class="[
      'base-button',
      `btn-${variant}`,
      `btn-size-${size}`,
      { 'is-loading': isLoading, 'is-disabled': disabled }
    ]"
    :disabled="disabled || isLoading"
    :title="title"
    @click="$emit('click', $event)"
  >
    <span v-if="isLoading" class="btn-spinner"></span>
    <slot name="icon-left" />
    <slot />
    <slot name="icon-right" />
  </button>
</template>

<script setup>
defineProps({
  type: {
    type: String,
    default: 'button',
  },
  variant: {
    type: String,
    default: 'secondary', // 'primary' | 'secondary' | 'danger' | 'danger-solid' | 'icon' | 'chip' | 'refresh' | 'quick-sync'
  },
  size: {
    type: String,
    default: 'md', // 'sm' | 'md' | 'lg'
  },
  disabled: {
    type: Boolean,
    default: false,
  },
  isLoading: {
    type: Boolean,
    default: false,
  },
  title: {
    type: String,
    default: '',
  },
});

defineEmits(['click']);
</script>

<style scoped>
.base-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 0.375rem;
  font-family: inherit;
  font-weight: 500;
  border-radius: var(--radius-md, 6px);
  cursor: pointer;
  transition: all 0.15s ease;
  user-select: none;
  white-space: nowrap;
}

.base-button:disabled {
  opacity: 0.55;
  cursor: not-allowed;
}

.btn-size-sm {
  font-size: 11px;
  padding: 0.25rem 0.5rem;
  height: 24px;
}

.btn-size-md {
  font-size: 12px;
  padding: 0.35rem 0.75rem;
  height: 28px;
}

.btn-size-lg {
  font-size: 13px;
  padding: 0.5rem 1rem;
  height: 34px;
}

.btn-spinner {
  width: 12px;
  height: 12px;
  border: 2px solid currentColor;
  border-top-color: transparent;
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}
</style>

