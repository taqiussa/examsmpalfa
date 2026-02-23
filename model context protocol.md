# Model Context Protocol

Dokumen ini merangkum pemahaman tentang repo `/home/taqius/rust/examsmkmifda` agar bisa dipakai sebagai konteks awal saat memulai chat dengan AI.

## Ringkasan Arsitektur
- Backend: Rust (Axum), Tera templating, SQLX (MySQL).
- Frontend: Vite + HTMX + AlpineJS + TailwindCSS.
- Template HTML: Tera (`templates/`).
- Build FE ke `static/` dengan manifest Vite.
- Docker multi-stage untuk FE build + Rust build.

## Struktur Folder Utama
- `src/`: backend Rust (routes, controllers, middlewares, models, utils, layouts, config).
- `fe/`: frontend Vite.
- `templates/`: Tera templates (layout, pages, components).
- `static/`: output build Vite (`static/.vite/manifest.json`).
- `migrations/`: migrasi database (fokus ujian).
- `Dockerfile`, `docker-compose.yml`, `docker-compose.prod.yml`.

## Alur Program (Request → Response)
1. `src/main.rs`: load env, connect DB, init Tera, register function `tahun_options`, setup router.
2. Router utama di `src/routes/web_routes.rs`:
   - `public_routes()` (guest) untuk login.
   - `authenticated` (auth) dengan `auth_middleware`.
   - Role-based: `admin_routes`, `guru_routes`, `siswa_routes` dengan `role_middleware`.
3. Rendering via `PageContext` (`src/utils/page_context.rs`) dan `render()` (`src/utils/render.rs`).

## Auth & Middleware
- Login: `src/controllers/auth/login_action.rs`.
  - Username/nis → query `users` → bcrypt verify → set cookie `user_id`.
- Logout: `src/controllers/auth/logout_action.rs`.
- Auth middleware: `src/middlewares/auth_middleware.rs`.
  - Ambil `user_id` cookie → query roles (`roles`, `model_has_roles`) → query `users` → inject `AuthUser`.
- Role middleware: `src/middlewares/role_middleware.rs`.
  - Cek `AllowedRoles`, render `errors/403.html` jika tidak allowed.
- Guest middleware: `src/middlewares/guest_middleware.rs`.
  - Redirect ke `/dashboard` jika sudah login.

## Layouting & Tera
- Layout utama:
  - `templates/layouts/app.html` (authenticated).
  - `templates/layouts/guest.html` (login).
- Sidebar role-based: `src/layouts/sidebar/sidebar.rs` + menu di `src/layouts/sidebar/menus/`.
- Render helper: `src/utils/render.rs`.
- Function Tera: `tahun_options` di `src/utils/functions.rs`.

## Frontend (Vite)
- Entry: `fe/src/main.ts` → load `app.css` + `app.js`.
- `fe/src/app.js`:
  - Alpine store flash.
  - HTMX hooks untuk loading.
  - `window.flash()` fallback.
- Vite config: `fe/vite.config.ts`.
  - Build output `static/` + `manifest: true`.
- Server inject assets:
  - `src/utils/vite.rs` (dev via `localhost:5173`, prod via manifest).

## Database dan Migrasi
Migrasi di `migrations/` fokus pada domain ujian:
1. `20240101000001_create_ujians_tables.sql`
   - `ujians`, `soals`, `ujian_soals`, trigger update `total_soal`.
2. `20260207000002_alter_soals_pertanyaan_longtext.sql`
   - `soals.pertanyaan` → LONGTEXT.
3. `20260211000003_create_ujian_peserta_tables.sql`
   - `ujian_tokens`, `ujian_pesertas`, `ujian_jawabans`.
4. `20260219000004_alter_soals_opsi_longtext.sql`
   - opsi → LONGTEXT, `kunci_jawaban` nullable.
5. `20260219000005_create_ujian_soals_if_missing.sql`
   - safety create `ujian_soals` + trigger.
6. `20260219000006_alter_ujian_jawabans_uraian.sql`
   - `pilihan` nullable, tambah `jawaban_uraian`.
7. `20260220000007_add_ujian_uraian_score.sql`
   - `nilai_uraian`, `status_uraian`.
8. `20260220000008_add_ujian_tahun.sql`
   - tambah `ujians.tahun` + index.
9. `20260220000009_backfill_ujian_tahun.sql`
   - backfill `tahun` dari `tanggal`.

Catatan: Tabel lain (`users`, `roles`, `model_has_roles`, `kelas`, `siswas`, `biodatas`, `absensis`, `mata_pelajarans`) tidak ada di migrasi repo ini, diasumsikan sudah ada di DB eksternal/legacy. Skema test ada di `src/test_support.rs`.

## Docker
- `Dockerfile`: multi-stage (FE build → Rust build → runtime).
- `docker-compose.yml`: build/pull image, port `8830:3000`.
- `docker-compose.prod.yml`: pull image, port `8830:${APP_PORT}`.

## Endpoint Utama (High Level)
- Public: `/login`.
- Auth common: `/`, `/dashboard`, `/logout`.
- Admin: `/tambah-pengguna`.
- Guru: `/absensi-kelas`, `/biodata-siswa`, `/ujian`, `/progress-ujian`, `/review-uraian`, `/hasil-nilai`.
- Siswa: `/siswa/ujian` + token flow.

## File Referensi Kunci
- `src/main.rs`
- `src/routes/web_routes.rs`
- `src/middlewares/auth_middleware.rs`
- `src/middlewares/role_middleware.rs`
- `src/controllers/auth/login_action.rs`
- `src/controllers/guru/ujian.rs`
- `src/controllers/siswa/ujian_siswa.rs`
- `fe/vite.config.ts`
- `src/utils/vite.rs`
- `templates/layouts/app.html`
