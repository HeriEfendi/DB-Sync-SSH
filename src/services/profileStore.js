/**
 * profileStore.js
 * Manages connection profiles and table selection templates
 */

const STORAGE_PROFILES_KEY = 'db_sync_ssh_profiles';
const STORAGE_ACTIVE_ID_KEY = 'db_sync_ssh_active_profile_id';

const DEFAULT_PROFILES = [
  {
    id: 'prof-staging-olshoperp2',
    name: 'staging_olshoperp2',
    db_type: 'MySQL / MariaDB',
    ssh: {
      host: 'olshoperp.com',
      port: 22,
      username: 'ubuntu',
      auth_method: 'key_no_pass',
      private_key_path: '/home/lenovo/.ssh/id_rsa',
      passphrase: '',
      password: '',
    },
    remote_db: {
      host: 'localhost',
      port: 3306,
      username: 'staging_user',
      password: '',
      database: 'staging_olshoperp2',
    },
    local_db: {
      host: '127.0.0.1',
      port: 3306,
      username: 'root',
      password: '',
      database: 'staging_olshoperp2',
      use_docker: false,
      docker_container: '',
    },
    templates: [
      {
        id: 'tpl-custom',
        name: 'Custom Selection',
        tableNames: [],
      },
    ],
  },
  {
    id: 'prof-tyas-olshoperp',
    name: 'tyas_olshoperp',
    db_type: 'MySQL / MariaDB',
    ssh: {
      host: 'tyas.olshoperp.com',
      port: 22,
      username: 'root',
      auth_method: 'key_no_pass',
      private_key_path: '/home/lenovo/.ssh/id_rsa',
      passphrase: '',
      password: '',
    },
    remote_db: {
      host: 'localhost',
      port: 3306,
      username: 'tyas_user',
      password: '',
      database: 'tyas_olshoperp',
    },
    local_db: {
      host: '127.0.0.1',
      port: 3306,
      username: 'root',
      password: '',
      database: 'tyas_olshoperp',
      use_docker: false,
      docker_container: '',
    },
    templates: [
      {
        id: 'tpl-custom-2',
        name: 'Custom Selection',
        tableNames: [],
      },
    ],
  },
];

export function getStoredProfiles() {
  try {
    const raw = localStorage.getItem(STORAGE_PROFILES_KEY);
    if (!raw) {
      saveProfiles(DEFAULT_PROFILES);
      return DEFAULT_PROFILES;
    }
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) && parsed.length > 0 ? parsed : DEFAULT_PROFILES;
  } catch (e) {
    console.error('Error loading profiles from localStorage:', e);
    return DEFAULT_PROFILES;
  }
}

export function saveProfiles(profiles) {
  try {
    localStorage.setItem(STORAGE_PROFILES_KEY, JSON.stringify(profiles));
  } catch (e) {
    console.error('Error saving profiles to localStorage:', e);
  }
}

export function getActiveProfileId() {
  try {
    const id = localStorage.getItem(STORAGE_ACTIVE_ID_KEY);
    return id || DEFAULT_PROFILES[0].id;
  } catch {
    return DEFAULT_PROFILES[0].id;
  }
}

export function setActiveProfileId(id) {
  try {
    localStorage.setItem(STORAGE_ACTIVE_ID_KEY, id);
  } catch (e) {
    console.error('Error saving active profile ID:', e);
  }
}
