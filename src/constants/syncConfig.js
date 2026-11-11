/**
 * Application constants for DB Sync SSH.
 * Complies with AGENTS.md: Ekstraksi nilai statis dan konfigurasi ke constants/.
 */

export const ROW_LIMIT_PRESETS = [
  { value: 0, label: 'Unlimited (Semua)' },
  { value: 1000, label: '1.000 Row' },
  { value: 10000, label: '10.000 Row' },
  { value: 50000, label: '50.000 Row' },
  { value: 100000, label: '100.000 Row' },
  { value: 500000, label: '500.000 Row' },
  { value: 1000000, label: '1.000.000 Row' },
  { value: 2000000, label: '2.000.000 Row' },
  { value: 5000000, label: '5.000.000 Row' },
  { value: 'custom', label: 'Custom...' },
];

export const WORKER_CONCURRENCY_OPTIONS = [
  { value: 1, label: '1 Worker' },
  { value: 2, label: '2 Workers' },
  { value: 3, label: '3 Workers' },
  { value: 4, label: '4 Workers' },
];

export const AUTO_SYNC_INTERVAL_OPTIONS = [
  { value: 0, label: 'Off (Manual)' },
  { value: 5, label: 'Tiap 5 Menit' },
  { value: 15, label: 'Tiap 15 Menit' },
  { value: 30, label: 'Tiap 30 Menit' },
  { value: 60, label: 'Tiap 1 Jam' },
  { value: 120, label: 'Tiap 2 Jam' },
];

export const SSH_AUTH_METHODS = [
  { value: 'key_no_pass', label: 'SSH Key (No Password)' },
  { value: 'key_with_passphrase', label: 'SSH Key with Passphrase' },
  { value: 'password', label: 'Password' },
  { value: 'agent', label: 'SSH Agent / Default Keys (~/.ssh)' },
];

export const DEFAULT_PORTS = {
  SSH: 22,
  MYSQL: 3306,
};

