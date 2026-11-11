<template>
  <label :class="['base-checkbox-label', { disabled }]">
    <input
      type="checkbox"
      class="base-checkbox-input"
      :checked="modelValue"
      :indeterminate.prop="indeterminate"
      :disabled="disabled"
      @change="$emit('update:modelValue', $event.target.checked)"
    />
    <span v-if="label || $slots.default" class="checkbox-text">
      <slot>{{ label }}</slot>
    </span>
  </label>
</template>

<script setup>
defineProps({
  modelValue: {
    type: Boolean,
    default: false,
  },
  indeterminate: {
    type: Boolean,
    default: false,
  },
  disabled: {
    type: Boolean,
    default: false,
  },
  label: {
    type: String,
    default: '',
  },
});

defineEmits(['update:modelValue']);
</script>

<style scoped>
.base-checkbox-label {
  display: inline-flex;
  align-items: center;
  gap: 0.35rem;
  font-size: 11.5px;
  cursor: pointer;
  user-select: none;
}

.base-checkbox-label.disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.base-checkbox-input {
  cursor: pointer;
  accent-color: var(--primary-green, #0e7548);
}

.checkbox-text {
  color: var(--text-secondary, #475569);
}
</style>

