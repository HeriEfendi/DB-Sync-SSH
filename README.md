# ⚡ DB Sync Engine (SSH)

A high-performance desktop database synchronization tool built with **Tauri 2.0**, **Vue 3**, and **Rust**.

Dirancang untuk menyinkronkan database MySQL / MariaDB remote secara aman melalui **SSH Port-Forwarding Tunnel** langsung ke instance MySQL lokal atau container Docker, dengan pemrosesan streaming in-memory, penggunaan RAM terikat (*bounded memory*), *multi-worker concurrency*, *adaptive virtual scrolling*, dan *zero-downtime keyset chunking*.

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
  * Tabel tanpa `updated_at` (atau watermark waktu yang valid) memakai full upsert dengan `REPLACE INTO` agar perubahan pada baris lama tidak terlewat.
  * Keyset incremental otomatis aktif untuk primary key integer tunggal, dan fallback aman untuk non-integer/composite key.

### 2. ⚡ Batch Remote Watermark Check (< 50 ms)
* **Mengeliminasi Round-Trip Latency SSH**:
  * Alih-alih memeriksa perubahan tabel satu per satu yang memakan waktu detik, engine mengelompokkan hingga 40 tabel per batch menggunakan kueri tunggal `UNION ALL`:
    ```sql
    (SELECT 'users' AS _tbl FROM `users` WHERE (id > 10500) LIMIT 1)
    UNION ALL
    (SELECT 'orders' AS _tbl FROM `orders` WHERE (id > 42000) LIMIT 1)
    ...
    ```
  * Tabel yang tidak memiliki data baru otomatis teridentifikasi dan di-skip secara instan (*"tek tek tek"* tanpa jeda) sebelum pipeline worker berjalan.
  * Dilengkapi fallback otomatis ke pengecekan individual jika server remote memiliki batasan kueri.

### 3. 🔍 Pre-Sync Diff & Row Count Inspector
* **Pengecekan Sebelum Sinkronisasi**:
  * Tombol **"Compare Diff"** di toolbar membandingkan jumlah baris remote vs MySQL lokal sebelum sync dijalankan.
  * Badge visual informatif:
    * 🟢 **In-Sync**: Baris remote dan lokal identik.
    * 🟡 **Diverged**: Ada data baru atau perbedaan jumlah baris di remote.
    * 🆕 **Remote Only**: Tabel belum ada di MySQL lokal.
  * Tombol aksi instan **"Sync Diverged"** untuk langsung menyinkronkan tabel-tabel yang berbeda saja.

### 4. 📜 Adaptive Virtual Scrolling (TableView.vue)
* **Performa 60 FPS untuk 100 s/d 10.000+ Tabel**:
  * Jika jumlah tabel $> 50$, virtual scroll aktif secara otomatis, hanya me-render $\approx 25 - 40$ elemen `<tr>` yang terlihat di viewport.
  * Menggunakan baris penyeimbang (*spacer rows*) proporsional dan buffer 15 baris untuk scrolling mulus tanpa flicker (*zero layout shift*).
  * Diintegrasikan dengan `ResizeObserver` dan scroll handler berbasis `requestAnimationFrame`.
  * Otomatis me-reset posisi scroll saat filter atau pencarian diubah.

### 5. ⚡ Single-Table Quick Sync (Action Row)
* **Sinkronisasi Instan 1 Tabel**:
  * Tombol petir ⚡ **Sync** di setiap baris tabel memungkinkan pengembang menyinkronkan 1 tabel tertentu langsung tanpa perlu mengubah seleksi checkbox tabel lainnya.

### 6. 📌 Mini Floating Sync Widget
* **Sinkronisasi Latar Belakang Tanpa Hambatan**:
  * Saat modal sinkronisasi di-*minimize*, widget mengambang modern muncul di pojok kanan bawah.
  * Menampilkan nama profil, progress bar, persentase selesai, tabel yang sedang berjalan, serta tombol **Buka (Maximize)** dan **Stop Sync**.
  * Pengembang bebas berpindah halaman, mengatur template, atau melihat riwayat saat sinkronisasi tetap berjalan di latar belakang.

### 7. ⏱️ Background Auto-Sync & Desktop Notification
* **Opsi Sinkronisasi Berkala (Cron-like)**:
  * Pilihan interval otomatis: Tiap 5 menit, 15 menit, 30 menit, 1 jam, atau 2 jam.
  * Dilengkapi badge countdown `⏱️ mm:ss` yang tersimpan otomatis di profil.
  * Integrasi **Desktop Notification OS** bawaan untuk memberi tahu pengguna saat sinkronisasi selesai atau terjadi galat.

### 8. 📊 Interactive History Detail Drawer
* **Panel Rincian Riwayat Interaktif**:
  * Mengklik baris riwayat di tab **Sync History** membuka side drawer detail.
  * Menampilkan statistik ringkasan, daftar catatan/error per tabel (jika ada kegagalan sebagian), serta daftar seluruh tabel tersinkron dengan filter pencarian.
  * Tombol **"Jalankan Ulang Sync Ini"** untuk langsung mengeksekusi konfigurasi tabel dari riwayat tersebut.

### 9. 💻 Peningkatan Terminal Log Console
* **Log Filter Tabs**: Filter instan untuk kategori *Semua*, *Error*, *Warning*, dan *Berhasil*.
* **Real-time Search**: Kolom pencarian log interaktif yang mencari kecocokan pada pesan dan detail error.
* **Salin & Ekspor File (.txt)**:
  * Tombol **Salin Log** ke clipboard.
  * Tombol **Ekspor .txt** untuk mengunduh berkas laporan log lengkap berformat rapi beserta metadata sesi.
* **Auto-Scroll Pause**: Berhenti otomatis saat pengguna menggulir ke atas untuk membaca pesan error, disertai tombol mengambang *"Scroll ke bawah"* untuk kembali ke posisi live.

### 10. 🗜️ Smart Low-CPU Compression & 512 KB Pipe Tuning
* **Low-CPU Compression (Zstandard / Protocol / SSH)**:
  * Otomatis menegosiasikan `--compression-algorithms=zstd,zlib,uncompressed` dengan `--zstd-compression-level=1` pada MySQL 8.0+ CLI (overhead CPU remote < 1%), atau fallback ke `--compress`.
  * Negosiasi kompresi transport SSH otomatis: `zlib@openssh.com`.
* **512 KB Kernel Pipe Buffer Tuning**:
  * Menggunakan `fcntl(fd, F_SETPIPE_SZ, 512 * 1024)` untuk memperbesar kapasitas pipa kernel Linux dari default 64 KB menjadi 512 KB.
  * Mengurangi ribuan *context switches* per detik saat aliran throughput tinggi.
* **MySQL Importer Boost Flags**:
  * `--init-command="SET SESSION foreign_key_checks=0; SET SESSION unique_checks=0; SET SESSION autocommit=1; SET SESSION sql_mode='';"`
  * `--quick`, `--max-allowed-packet=1024M`, `--net-buffer-length=1M`.

### 11. ⚙️ Konfigurasi Concurrency Multi-Worker
* **1 – 4 Download Workers**:
  * Pengguna dapat memilih jumlah paralel worker remote langsung melalui dropdown di toolbar (1, 2, 3, atau 4 worker bersamaan) sesuai kapasitas CPU/jaringan server remote.

### 12. ⌨️ Pintasan Keyboard (Keyboard Shortcuts)
| Shortcut | Aksi | Lokasi |
| :--- | :--- | :--- |
| **`Ctrl + F` / `Cmd + F`** | Fokus ke kolom pencarian tabel | Daftar Tabel |
| **`Ctrl + F` / `Cmd + F`** | Fokus ke kolom pencarian log | Modal Log Progres |
| **`Ctrl + A` / `Cmd + A`** | Pilih semua (*Select All*) tabel hasil filter | Daftar Tabel |
| **`Ctrl + Enter` / `Cmd + Enter`** | Mulai sinkronisasi (*Start Sync*) tabel terpilih | Daftar Tabel |
| **`Ctrl + S` / `Cmd + S`** | Ekspor log sinkronisasi ke file `.txt` | Modal Log Progres |
| **`Esc`** | Membersihkan pencarian / melepas fokus | Daftar Tabel |
| **`Esc`** | Menutup modal Diff Inspector | Modal Diff |
| **`Esc`** | Menutup drawer detail riwayat | Tab Riwayat |
| **`Esc`** | Menutup atau minimize modal progres | Modal Log Progres |

---

## 🛠️ Alur Arsitektur Sinkronisasi

```
  +-------------------------------------------------------------+
  |                   Remote MySQL Database                     |
  |                (Via SSH Port-Forward Tunnel)                |
  +------------------------------+------------------------------+
                                 |
            +--------------------+--------------------+
            |                    |                    |
            v                    v                    v
  +--------------------+ +--------------------+ +--------------------+
  | SSH Worker 1 (Rust)| | SSH Worker 2 (Rust)| | SSH Worker N (Rust)|
  | - Keyset Chunking  | | - Keyset Chunking  | | - Keyset Chunking  |
  | - Zstd / Compress  | | - Zstd / Compress  | | - Zstd / Compress  |
  +---------+----------+ +---------+----------+ +---------+----------+
            |                    |                    |
            +--------------------+--------------------+
                                 |
                                 v
               +---------------------------------------+
               |   Bounded Channel: mpsc::channel(4)   |
               |  (256 KB Chunks, Max ~4 MB in flight) |
               +-------------------+-------------------+
                                   |
                                   v
               +---------------------------------------+
               |    Local MySQL Importer (Consumer)    |
               | - 512 KB Tuned Pipe STDIN Buffer      |
               | - Zero Temp Disk I/O (Pure Streaming) |
               | - Boost Flags (FK=0, Unique=0, Auto=1)|
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
   * Binary `mysql` atau `mariadb` CLI terpasang pada `PATH` (atau engine Docker aktif jika menggunakan mode container).
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
GitHub Actions membangun installer Linux (`.deb`, `.rpm`, `.pkg.tar.zst` untuk Arch Linux), Windows (`.msi`, `.exe`), dan macOS (`.dmg`) saat tag versi `vX.Y.Z` didorong ke GitHub:

```bash
npm run release -- 1.2.3
```

---

## 📝 Format Konfigurasi Profil

Aplikasi secara otomatis mengelola profil database lokal dan remote yang tersimpan aman di direktori data aplikasi pengguna (`~/.local/share/db-sync-ssh/` di Linux):

* **SSH Host / Port / User**: Akses ke server tujuan (misal `103.x.x.x:22`).
* **SSH Auth**: Mendukung *Password*, *Private Key File* (dengan atau tanpa passphrase), dan *SSH Agent*.
* **Remote DB**: User, password, nama database, dan port di remote MySQL.
* **Local DB**: Konfigurasi koneksi MySQL lokal atau nama container Docker.
* **Templates**: Simpan grup seleksi tabel tertentu untuk sinkronisasi berulang.

---

## 📄 Lisensi

Didistribusikan di bawah lisensi MIT. Silakan gunakan dan sesuaikan untuk kebutuhan tim atau proyek Anda.
