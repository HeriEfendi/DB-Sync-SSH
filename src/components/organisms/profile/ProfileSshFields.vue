<template>
  <div class="form-section">
    <div class="section-title-row">
      <Shield :size="14" />
      <span>SSH Configuration</span>
    </div>
    <div class="form-grid">
      <div class="form-group">
        <label class="form-label">SSH Host *</label>
        <input
          v-model="ssh.host"
          type="text"
          class="form-input"
          placeholder="123.45.67.89 or example.com"
          required
        />
      </div>
      <div class="form-group">
        <label class="form-label">SSH Port *</label>
        <input
          v-model.number="ssh.port"
          type="number"
          class="form-input"
          placeholder="22"
          required
        />
      </div>
      <div class="form-group">
        <label class="form-label">SSH Username *</label>
        <input
          v-model="ssh.username"
          type="text"
          class="form-input"
          placeholder="ubuntu, root, etc."
          required
        />
      </div>
      <div class="form-group">
        <label class="form-label">Auth Method *</label>
        <select v-model="ssh.auth_method" class="form-select">
          <option
            v-for="method in SSH_AUTH_METHODS"
            :key="method.value"
            :value="method.value"
          >
            {{ method.label }}
          </option>
        </select>
      </div>

      <!-- Key Path if Key Auth -->
      <div
        v-if="ssh.auth_method === 'key_no_pass' || ssh.auth_method === 'key_with_passphrase'"
        class="form-group full-width"
      >
        <label class="form-label">Private Key Path *</label>
        <div class="input-browse-group">
          <input
            v-model="ssh.private_key_path"
            type="text"
            class="form-input"
            placeholder="/home/user/.ssh/id_rsa"
          />
          <button type="button" class="btn-secondary" @click="$emit('browse-key')">
            Browse
          </button>
        </div>
      </div>

      <!-- Passphrase if Key with Passphrase -->
      <div
        v-if="ssh.auth_method === 'key_with_passphrase'"
        class="form-group full-width"
      >
        <label class="form-label">Key Passphrase</label>
        <input
          v-model="ssh.passphrase"
          type="password"
          class="form-input"
          placeholder="Passphrase private key"
        />
      </div>

      <!-- SSH Password if Password Auth -->
      <div
        v-if="ssh.auth_method === 'password'"
        class="form-group full-width"
      >
        <label class="form-label">SSH Password *</label>
        <input
          v-model="ssh.password"
          type="password"
          class="form-input"
          placeholder="Password user SSH"
        />
      </div>
    </div>
  </div>
</template>

<script setup>
import { Shield } from 'lucide-vue-next';
import { SSH_AUTH_METHODS } from '../../../constants/syncConfig.js';

defineProps({
  ssh: {
    type: Object,
    required: true,
  },
});

defineEmits(['browse-key']);
</script>

