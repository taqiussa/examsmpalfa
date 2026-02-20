# Template Rust Axum + Htmx + AlpineJs + TailwindCSS V3 - Rust Web Application
-- U
Aplikasi Template Untuk Dashboard Admin. Dibangun dengan Rust (Axum), MySQL, dan Vite + Htmx AlpineJs untuk frontend dan Tailwindcss V3 (Tera Templating)

---

## 📋 Prasyarat

Pastikan Anda telah menginstal:
- **Rust** (1.92.0 atau lebih baru) - [Install Rust](https://www.rust-lang.org/tools/install)
- **Node.js** (20 atau lebih baru) - [Install Node.js](https://nodejs.org/)
- **MySQL** (5.7 atau lebih baru)
- **Git**
- **Docker & Docker Compose** (untuk production)

---

## 🚀 Setup Awal

### 1. Clone Repository

```bash
git clone https://github.com/taqiussa/rust-htmx-template.git namafolder
cd namafolder
```

### 2. Konfigurasi Environment

```bash
cp .env.example .env
```

Edit file `.env` dan sesuaikan konfigurasi Anda:

```dotenv
APP_ENV=development
APP_PORT=3000
DATABASE_URL=mysql://root:password@localhost:3306/databasename
```

Sesuaikan:
- `APP_ENV` production dan development sesuaikan
- `DATABASE_URL` dengan kredensial MySQL Anda
- `APP_PORT` jika menggunakan port yang berbeda

### 3. Setup Database

Buat database MySQL dengan nama yang sama dengan yang ada di `DATABASE_URL`:

```bash
mysql -u root -p
```

```sql
CREATE DATABASE databasename CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;
```

---

## 💻 Development (Hot Reload)

### Satu Command (Frontend + Backend)

```bash
# Jalankan FE + BE dari root
cargo dev
# atau
cargo run --bin watch
```

> Pastikan `cargo-watch` sudah ter-install dan dependencies FE sudah di-install.

### Terminal 1: Backend (Rust)

```bash
# Install Rust dependencies
cargo install


### Terminal 2: Frontend (Vite + Tailwind)

```bash
cd fe

# Install dependencies
npm install

# Start development server dengan hot reload
cd ..
cargo dev
```

Frontend akan berjalan di `http://localhost:5173` (atau port yang ditampilkan)
Backend akan berjalan di `http://localhost:3000`
Hot Reload backend dan front sudah dalam command npm run dev
Cek Package.jsons

---

## 🏗️ Production dengan Docker

### Prerequisites
- Docker daemon berjalan
- Network `proxy_network` sudah dibuat (untuk Nginx Proxy Manager)

### 1. Buat Network (jika belum ada)

```bash
docker network create proxy_network
```

### 2. Opsi Deploy

Ada dua cara deploy:
- **Opsi A (Pull image dari Docker Hub)**: build dilakukan di lokal/CI, server hanya pull.
- **Opsi B (Build langsung di server)**: server build image dari source.

#### Opsi A: Pull Image (Direkomendasikan)
1. **Build & Push image (di lokal/CI)**
```bash
docker build -t yourdockerhubuser/examsmkmifda:latest .
docker push yourdockerhubuser/examsmkmifda:latest
```

2. **Siapkan `.env.prod` di server**
File ini **disimpan di server**, bukan di Docker Hub.  
Contoh isi:
```
APP_ENV=production
APP_PORT=3000
DATABASE_URL=mysql://user:pass@mariadb:3306/examsmkmifda
```

3. **Pull & Run di server**
```bash
docker compose --env-file .env.prod pull
docker compose --env-file .env.prod up -d
```

Catatan:
- Kamu tetap **perlu `git clone` repo di server** supaya ada `docker-compose.yml`.
- Server **tidak perlu build** ulang image kalau sudah pull dari Docker Hub.

#### Opsi B: Build di Server
1. **Clone repo di server**
```bash
git clone https://github.com/taqiussa/rust-htmx-template.git namafolder
cd namafolder
```
2. **Siapkan `.env.prod` di server** (seperti contoh di atas).
3. **Build & Run**
```bash
docker compose --env-file .env.prod up -d --build
```

### Catatan SQLX saat build
Build image **tidak butuh koneksi DB** jika `.sqlx/` sudah ada dan `SQLX_OFFLINE=true` dipakai.  
Jalankan ini di lokal/CI setiap kali query SQL berubah:
```bash
DATABASE_URL="mysql://user:pass@host:3306/db" 
cargo sqlx prepare -- --bin examsmkmifda
```

### 3. Henti Services

```bash
docker compose down
```

---

## 📁 Struktur Project

```
namafolder/
├── src/                 # Rust backend source
│   ├── main.rs
│   ├── controllers/     # Business logic
│   ├── models/          # Data structures
│   ├── routes/          # API endpoints
│   ├── middlewares/      # Auth, role, etc
│   ├── config/          # Database config
│   └── utils/           # Helper functions
├── fe/                  # Vite + Htmx + AlpineJs + TailwindCss V3 frontend
│   ├── src/
│   ├── package.json
│   ├── tsconfig.json
│   └── vite.config.ts
├── templates/           # Askama HTML templates
├── static/              # Built frontend files
├── Cargo.toml          # Rust dependencies
├── docker-compose.yml  # Docker configuration
├── Dockerfile          # Docker build steps
└── .env.example        # Environment variables template
```

---

## 🔐 Network Configuration (Docker)

File `docker-compose.yml` menggunakan external network `proxy_network` untuk komunikasi dengan Nginx Proxy Manager:

```yaml
services:
  rust:
    build:
      context: .
      dockerfile: Dockerfile
    container_name: namacontainer
    restart: unless-stopped
    env_file:
      - .env
    networks:
      - proxy_network  # Connect ke network proxy
    ports:
      - "3000:3000"

networks:
  proxy_network:
    external: true  # Network sudah dibuat sebelumnya
```

**Port Mapping:**
- `3000` (host) → `3000` (container)

Jika mengubah port, sesuaikan di:
1. `.env` → `APP_PORT=3000`
2. `docker-compose.yml` → `ports: - "3000:3000"`
3. Nginx Proxy Manager configuration

---

## 📝 Environment Variables

| Variable | Deskripsi | Contoh |
|----------|-----------|---------|
| `APP_ENV` | Environment mode | `development` / `production` |
| `APP_PORT` | Port backend | `3000` |
| `DATABASE_URL` | MySQL connection string | `mysql://user:pass@host:3306/db` |
| `PULL_POLICY` | Pull policy image | `never` / `if_not_present` / `always` |

Catatan:
- File `.env.prod` atau `.env.local` dibaca lewat `--env-file`.
- Nilai dari file tersebut diteruskan ke container melalui `environment` di `docker-compose.yml`.
- Port container mengikuti `APP_PORT` (mapping host: `8950` → container: `APP_PORT`).

---

## 🛠️ Build Frontend untuk Production

```bash
cd fe
npm run build
```

Build output akan ditempatkan di `../static/` dan secara otomatis disertakan dalam Docker image.

### Catatan Tailwind + Tera (Penting)
Tailwind membaca class dari file HTML template. Karena template Tera berada di `templates/`,
pastikan Docker build **menyalin folder `templates/` ke stage frontend builder**.
Tanpa ini, Tailwind akan melakukan purge dan hasil CSS jadi kosong/mini di production.

Di `Dockerfile` sudah ditambahkan:
```
COPY templates/ ../templates/
```

---

## 📦 Dependencies Utama

**Backend (Rust):**
- Axum - Web framework
- SQLx - Async database
- Askama - Template engine
- Jsonwebtoken - JWT authentication
- Bcrypt - Password hashing

**Frontend (Node.js):**
- Vite - Build tool
- Htmx + AlpineJs
- Tailwind CSS - Styling

---

## 🐛 Troubleshooting

### Error: "Database connection refused"
- Pastikan MySQL berjalan
- Verifikasi `DATABASE_URL` di `.env`
- Buat database jika belum ada

### Error: "Port already in use"
- Ubah `APP_PORT` di `.env`
- Atau stop aplikasi lain yang menggunakan port tersebut

### Frontend tidak ter-update saat development
- Pastikan `npm run dev` sudah berjalan
- Clear browser cache (Ctrl+Shift+Delete)
- Restart Vite dev server

### Docker build gagal
- Jalankan dengan `--no-cache` flag
- Pastikan Docker daemon berjalan
- Cek disk space yang tersedia

---

## 📞 Support

Untuk pertanyaan atau issues, buat issue di GitHub repository ini.

---

## 📄 License

MIT License - lihat LICENSE file untuk detail lengkap.
