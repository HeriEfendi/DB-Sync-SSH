# ⚡ DB Sync Engine (SSH)

A high-performance desktop database synchronization tool built with **Tauri 2.0**, **Vue 3**, and **Rust**.

Designed to synchronize remote MySQL / MariaDB databases securely across **SSH tunnels** directly into a local MySQL instance or Docker container, with zero web-server CPU overhead, bounded RAM usage, multi-worker concurrency, and instant keyset streaming.

---

## 🚀 Fitur Unggulan Terbaru

### 1. 🔄 Dual Synchronization Modes
* **Fresh Sync**:
  * Melakukan reset penuh tabel di MySQL lokal (`DROP TABLE IF EXISTS` lalu `CREATE TABLE`).
  * Mengalirkan seluruh data dari baris awal menggunakan `INSERT INTO` ber-chunk.
  * Sangat cocok untuk inisialisasi awal database lokal dari server staging/production.
* **Sync New & Update (Incremental)**:
  * Mempertahankan struktur yang ada (`CREATE TABLE IF NOT EXISTS`).
  * Menggunakan watermark **Last ID** dan waktu server remote. Tabel dengan kolom `updated_at` menyalin baris baru serta perubahan yang dibuat sejak sinkronisasi berhasil sebelumnya.
  * Tabel tanpa `updated_at` (atau watermark waktu yang valid) memakai full upsert dengan `REPLACE INTO` agar perubahan pada baris lama tidak terlewat; ini lebih akurat tetapi dapat mentransfer lebih banyak data.

### 2. ⚡ Multi-Worker Pipeline Architecture
Mengadopsi pola asinkron tingkat tinggi (Producer-Consumer over SSH):
* **2 Concurrent Remote SSH Workers (Producers)**:
  * Mengambil struktur DDL dan chunk data tabel secara paralel langsung melalui SSH port-forwarding tunnel.
  * Mengisi bounded channel queue (`tokio::sync::mpsc::channel(4)`) untuk memangkas latensi transfer data SSH.
* **1 Dedicated Local Importer Worker (Consumer)**:
  * Membaca stream DDL dan data chunk dari channel queue dan mengalirkannya langsung ke STDIN child process `mysql` lokal.
  * **Zero Temporary Disk I/O**: Dump SQL tidak pernah ditulis ke file disk lokal, melainkan langsung dialirkan secara *in-memory streaming*.
* **Bounded RAM Buffer**:
  * Antrian channel dibatasi hingga 4 chunk buffer sehingga konsumsi memori desktop tetap stabil dan ringan (< 60 MB RAM) meskipun menyinkronkan database berukuran puluhan gigabytes.

### 3. 🧩 Keyset Pagination Chunking (50.000 Baris / Chunk)
* **Bebas Beban Server Remote (No `COUNT(*)` Overhead)**:
  * Menghilangkan query `SELECT COUNT(*)` lambat yang biasanya mengunci CPU database remote saat menangani tabel raksasa.
* **Covering Index Boundary Discovery**:
  * Mencari batas atas tiap chunk menggunakan index-only scan yang selesai dalam hitungan milidetik (< 10 ms):
    ```sql
    SELECT MAX(t.pk) FROM (
      SELECT `id` AS pk FROM `nama_tabel`
      WHERE `id` > current_last_id
      ORDER BY `id` ASC LIMIT 50000
    ) AS t;
    ```
  * Query slice data aman: `WHERE pk > start_id AND pk <= chunk_max`.
* **Auto Primary Key Detection**:
  * Otomatis mendeteksi kolom Primary Key tabel via `information_schema.COLUMNS`.
  * Tabel tanpa integer/single Primary Key otomatis dialirkan secara aman melalui standard streaming fallback.

### 4. 💾 State Watermarking & History Tracking
* **Pencatatan Last ID & Last Sync**:
  * Menyimpan status watermark per-tabel (`last_id`, `primary_key`, `mode`, `last_sync_at`) di SQLite lokal.
  * Tampilan tabel luar memisahkan kolom **Last ID** dan **Last Sync** secara mandiri dan rapi.
* **Riwayat Sinkronisasi (Sync History)**:
  * Mencatat status eksekusi (*Completed*, *Aborted*, *Failed*), jumlah tabel, baris tersalin, dan durasi.
  * Desain tabel riwayat yang padat dan informatif.

### 5. 🔒 Transport & Keamanan SSH
* Mendukung autentikasi SSH lengkap:
  * **SSH Key (Tanpa Passphrase maupun dengan Passphrase)** disertai file picker native.
  * **Password Authentication**.
  * **SSH Agent / Default System Keys** (`~/.ssh/id_rsa`, `~/.ssh/id_ed25519`).
* Auto-reconnect & safe cleanup: Otomatis memutus port forwarding dan menonaktifkan child process saat sinkronisasi dibatalkan.

### 6. 🖥️ Antarmuka Pengguna Modern & Ringan
* **Sticky Navigation & Layout**:
  * Header koneksi dan bottom bar tindakan tetap berada di posisi statis (tidak ikut tergulung saat scroll tabel panjang).
* **Auto-Sync DDL di Background**:
  * Checkbox **Structure** diset terkunci (`checked` dan `disabled`), menandakan DDL selalu disinkronkan secara otomatis.
* **Expanded Progress Console**:
  * Modal progress berukuran luas (`max-width: 980px`) dengan tinggi terminal log `380px` (hingga `52vh`) untuk memantau aktivitas 2 worker secara live.
* **Dukungan Lingkungan Lokal**:
  * Mendukung koneksi MySQL/MariaDB native maupun **Docker Container** (`docker exec -i <container> mysql`).

---

## 🛠️ Alur Arsitektur Sinkronisasi

```
  +-------------------------------------------------------------+
  |                   Remote MySQL Database                     |
  |                (Via SSH Port-Forward Tunnel)                |
  +------------------------------+------------------------------+
                                 |
           +---------------------+---------------------+
           |                                           |
           v                                           v
+-----------------------+                   +-----------------------+
|  SSH Worker 1 (Rust)  |                   |  SSH Worker 2 (Rust)  |
| - Fast Keyset Chunk   |                   | - Fast Keyset Chunk   |
| - DDL & Data Dump     |                   | - DDL & Data Dump     |
+-----------+-----------+                   +-----------+-----------+
            |                                           |
            +---------------------+---------------------+
                                  |
                                  v
              +---------------------------------------+
              |   Bounded Channel: mpsc::channel(4)   |
              |   (Prevent RAM spike, Backpressure)   |
              +-------------------+-------------------+
                                  |
                                  v
              +---------------------------------------+
              |    Local MySQL Importer (Consumer)    |
              | - Non-blocking stream directly to     |
              |   STDIN process (Native / Docker CLI) |
              +-------------------+-------------------+
                                  |
                                  v
              +---------------------------------------+
              |         Local Database Engine         |
              |   (Native MySQL / Docker Container)   |
              +---------------------------------------+
```

---

## 📋 Prasyarat Sistem

1. **Rust & Cargo**: v1.75 ke atas ([rustup.rs](https://rustup.rs/))
2. **Node.js**: v18 ke atas & `npm` / `pnpm`
3. **Klien MySQL Lokal**:
   * Binary `mysql` CLI terpasang pada `PATH` (atau engine Docker aktif jika menggunakan mode container).
4. **Sistem Operasi**: Linux, macOS, atau Windows.

---

## 💻 Menjalankan & Membangun Aplikasi

### Mode Pengembangan (Development):
```bash
# Instalasi dependensi frontend
npm install

# Jalankan dev server Vite + Tauri GUI
npm run tauri dev
```

### Build Binary Siap Pakai (Production Release):
```bash
npm run tauri build
```
Paket installer dan binary executable akan dihasilkan di folder:
`src-tauri/target/release/bundle/` (`.deb`, `.rpm`, `.AppImage`, atau `.exe`/`.dmg`).

### Release Otomatis via GitHub Actions

GitHub Actions membangun installer Linux (`.deb`, `.rpm`, `.pkg.tar.zst` untuk Arch Linux), Windows (`.msi`, `.exe`), dan macOS (`.dmg`) saat tag versi `vX.Y.Z` didorong ke GitHub. Untuk membuat release, pastikan seluruh perubahan sudah di-commit dan branch aktif sudah siap dirilis, lalu jalankan:

```bash
npm run release -- 1.2.3
```

Perintah tersebut menyamakan versi di manifest frontend dan Tauri/Rust, membuat commit serta tag release, lalu mendorong branch dan tag ke remote `origin`. Workflow akan membuat GitHub Release dan mengunggah paket hasil build. Versi harus dinaikkan sebelum membuat release berikutnya. Untuk mengulang build pada tag yang sudah ada tanpa memindahkan tag, jalankan workflow **Release Desktop Apps** secara manual dari branch terbaru dan isi input `tag` (misalnya `v0.1.1`).

---

## 📝 Format Konfigurasi Profil

Aplikasi secara otomatis mengelola profil database lokal dan remote yang tersimpan terenkripsi/aman di direktori data aplikasi pengguna (`~/.local/share/db-sync-ssh/` di Linux).

Contoh pengaturan profil:
* **SSH Host / Port / User**: Akses ke server tujuan (misal `103.x.x.x:22`).
* **SSH Auth**: Pilih *Password* atau *Private Key File*.
* **Remote DB**: User, password, dan nama database di remote MySQL.
* **Local DB**: Konfigurasi koneksi MySQL lokal atau nama container Docker.

---

## 📄 Lisensi

Didistribusikan di bawah lisensi MIT. Silakan gunakan dan sesuaikan untuk kebutuhan tim atau proyek Anda.
