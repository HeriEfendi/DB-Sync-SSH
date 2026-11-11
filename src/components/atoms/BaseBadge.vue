<template>
  <span :class="['base-badge', `badge-${variant}`, { 'has-dot': dot }]">
    <span v-if="dot" class="badge-dot" />
    <slot name="icon" />
    <slot />
  </span>
</template>

<script setup>
defineProps({
  variant: {
    type: String,
    default: 'neutral', // 'success' | 'warning' | 'danger' | 'info' | 'neutral'
  },
  dot: {
    type: Boolean,
    default: false,
  },
});
</script>

<style scoped>
.base-badge {
  display: inline-flex;
  align-items: center;
  gap: 0.35rem;
  padding: 0.15rem 0.45rem;
  font-size: 11px;
  font-weight: 500;
  border-radius: var(--radius-sm, 4px);
  white-space: nowrap;
}

.badge-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background-color: currentColor;
}

.badge-success {
  background-color: var(--success-green-bg, #ecfdf5);
  color: var(--success-green, #10b981);
}

.badge-warning {
  background-color: var(--warning-amber-bg, #fffbeb);
  color: var(--warning-amber, #f59e0b);
}

.badge-danger {
  background-color: var(--danger-red-bg, #fef2f2);
  color: var(--danger-red, #ef4444);
}

.badge-info {
  background-color: #eff6ff;
  color: #3b82f6;
}

.badge-neutral {
  background-color: var(--bg-card-hover, #f1f5f9);
  color: var(--text-secondary, #475569);
}
</style>

