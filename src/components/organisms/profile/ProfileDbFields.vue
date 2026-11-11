<template>
  <div>
    <!-- Remote Database Configuration -->
    <div class="form-section">
      <div class="section-title-row">
        <Server :size="14" />
        <span>Remote Database Configuration</span>
      </div>
      <div class="form-grid">
        <div class="form-group">
          <label class="form-label">Database Host (relative to server) *</label>
          <input
            v-model="remoteDb.host"
            type="text"
            class="form-input"
            placeholder="localhost"
            required
          />
        </div>
        <div class="form-group">
          <label class="form-label">Database Port *</label>
          <input
            v-model.number="remoteDb.port"
            type="number"
            class="form-input"
            placeholder="3306"
            required
          />
        </div>
        <div class="form-group">
          <label class="form-label">Database User *</label>
          <input
            v-model="remoteDb.username"
            type="text"
            class="form-input"
            placeholder="db_user"
            required
          />
        </div>
        <div class="form-group">
          <label class="form-label">Database Password</label>
          <input
            v-model="remoteDb.password"
            type="password"
            class="form-input"
            placeholder="db_password"
          />
        </div>
        <div class="form-group full-width">
          <label class="form-label">Database Name *</label>
          <input
            v-model="remoteDb.database"
            type="text"
            class="form-input"
            placeholder="olshoperp_staging"
            required
          />
        </div>
      </div>
    </div>

    <!-- Local Database Configuration -->
    <div class="form-section">
      <div class="section-title-row">
        <HardDrive :size="14" />
        <span>Local Database Configuration</span>
      </div>
      <div class="form-grid">
        <div class="form-group">
          <label class="form-label">Local Host *</label>
          <input
            v-model="localDb.host"
            type="text"
            class="form-input"
            placeholder="127.0.0.1"
            required
          />
        </div>
        <div class="form-group">
          <label class="form-label">Local Port *</label>
          <input
            v-model.number="localDb.port"
            type="number"
            class="form-input"
            placeholder="3306"
            required
          />
        </div>
        <div class="form-group">
          <label class="form-label">Local User *</label>
          <input
            v-model="localDb.username"
            type="text"
            class="form-input"
            placeholder="root"
            required
          />
        </div>
        <div class="form-group">
          <label class="form-label">Local Password</label>
          <input
            v-model="localDb.password"
            type="password"
            class="form-input"
            placeholder=""
          />
        </div>
        <div class="form-group full-width">
          <label class="form-label">Local Database Name *</label>
          <input
            v-model="localDb.database"
            type="text"
            class="form-input"
            placeholder="staging_olshoperp2"
            required
          />
        </div>

        <div class="form-group full-width" style="margin-top: 0.5rem">
          <label class="check-label">
            <input v-model="localDb.use_docker" type="checkbox" />
            <span>Use Docker Container for MySQL Local</span>
          </label>
        </div>

        <div v-if="localDb.use_docker" class="form-group full-width">
          <label class="form-label">Docker Container Name *</label>
          <input
            v-model="localDb.docker_container"
            type="text"
            class="form-input"
            placeholder="e.g. mysql_staging or my-mariadb"
          />
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { Server, HardDrive } from 'lucide-vue-next';

defineProps({
  remoteDb: {
    type: Object,
    required: true,
  },
  localDb: {
    type: Object,
    required: true,
  },
});
</script>

