<template>
  <div
    v-if="profile"
    class="confirm-modal-overlay"
    @click.self="$emit('cancel')"
  >
    <div class="confirm-modal-box">
      <div class="confirm-modal-header">
        <AlertTriangle :size="20" class="confirm-warning-icon" />
        <h3 class="confirm-title">Hapus Koneksi?</h3>
      </div>
      <p class="confirm-message">
        Apakah Anda yakin ingin menghapus koneksi <strong>"{{ profile.name }}"</strong>? Tindakan ini tidak dapat dibatalkan.
      </p>
      <div class="confirm-actions">
        <button type="button" class="btn-secondary" @click="$emit('cancel')">
          Batal
        </button>
        <button type="button" class="btn-danger-solid" @click="$emit('confirm')">
          Hapus
        </button>
      </div>
    </div>
  </div>
</template>

<script setup>
import { AlertTriangle } from 'lucide-vue-next';

defineProps({
  profile: {
    type: Object,
    default: null,
  },
});

defineEmits(['cancel', 'confirm']);
</script>

