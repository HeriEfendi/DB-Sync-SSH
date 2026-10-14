# DB Sync Engine (SSH)

A high-performance desktop database synchronization tool built with **Tauri 2.0**, **Vue 3**, and **Rust**. 

Designed to sync remote MySQL / MariaDB databases securely across **SSH tunnels** directly into a local MySQL instance or Docker container, with zero web-server CPU overhead and instant streaming speeds.

---

## 🚀 Fitur Utama

- 🔒 **Secure SSH Transport**:
  - Koneksi langsung melalui SSH port forwarding tunnel (`ssh2`) dengan dukungan:
    - **SSH Key** (No Password / Passphrase) dengan file picker native.
    - **Password Authentication**.
    - **SSH Agent / Default Keys** (`~/.ssh`).
- ⚡ **Direct Stream Pipeline**:
  - Mengalirkan dump SQL secara langsung via child process STDIN ke MySQL lokal (`mysql`/`mariadb`) atau container Docker (`docker exec -i <container> mysql`).
  - Zero disk I/O untuk file sementara: data mengalir langsung dari SSH stream ke database lokal.
- 📋 **Preset Profiles & Multi-Table Selection**:
  - Simpan dan kelola profil koneksi remote & lokal (seperti `staging_olshoperp2`, `tyas_olshoperp`).
  - Filter pencarian tabel instan (`Ctrl+F`).
  - Mode seleksi: *Data + Structure* atau *Structure Only*, dapat diatur per-tabel maupun global.
  - Opsi *Row Limit* untuk sampling data staging/development.
  - Template pilihan tabel kustom (*Save As New*).
- 📜 **Sync History & Metrics**:
  - Menampilkan riwayat sinkronisasi lengkap: Status (*Completed*, *Aborted*, *Failed*), jumlah tabel, baris yang disalin, durasi, dan timestamp.
- 📊 **Real-time Live Progress & Terminal Logs**:
  - Progress bar dinamis, penghitung baris masuk live, dan terminal log console interaktif.
  - Tombol **Stop Sync** dengan terminasi proses yang bersih dan aman.

---

## 🛠️ Arsitektur Sistem

```
+------------------------------------+
|       Remote MySQL Server          |
|  (Host: 127.0.0.1 / Remote Host)   |
+-----------------+------------------+
                  |
                  | SSH Direct-TCPIP Tunnel / Remote mysqldump
                  v
+------------------------------------+
|       Rust Tauri 2.0 Engine        |
|  1. SSH Session Handshake & Auth   |
|  2. Async Port-Forward Listener    |
|  3. SQL DDL Fetcher & Stream Pipe  |
+-----------------+------------------+
                  |
     Direct STDIN Pipe to Native CLI
                  v
+------------------------------------+
|       Local Database Engine        |
| - Native MySQL / MariaDB Client    |
|   OR                               |
| - Docker Container Engine          |
+------------------------------------+
```

---

## 💻 Menjalankan Aplikasi

### Mode Pengembangan (Dev):
```bash
npm run tauri dev
```

### Build Binary Desktop (Release):
```bash
npm run tauri build
```
Hasil binary installer berekstensi `.deb`, `.rpm`, `.AppImage`, atau Arch Linux pacman akan dihasilkan di dalam folder `src-tauri/target/release/bundle/`.
