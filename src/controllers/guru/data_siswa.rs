use axum::{
    Extension, Form,
    extract::{Path, Query},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect},
};
use bcrypt::{DEFAULT_COST, hash};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, MySql, MySqlPool, Transaction};
use std::collections::HashMap;
use tera::Context;

use crate::{
    controllers::guru::absensi_kelas::Htmx,
    utils::{page_context::PageContext, render::render, tahun::data_tahun},
};

const DEFAULT_PASSWORD: &str = "12345678";
const MAX_NIS: u32 = 16_777_215;

#[derive(Serialize, FromRow)]
struct SiswaRow {
    id: i64,
    nis: String,
    nama: String,
    kelas: String,
}

#[derive(Serialize, FromRow, Clone)]
struct KelasOption {
    id: i64,
    nama: String,
    tingkat: String,
}

#[derive(Serialize)]
struct SiswaListData {
    kelas: Vec<KelasOption>,
    kelas_id: Option<i64>,
    tahun: String,
    list_tahun: Vec<String>,
}

#[derive(Deserialize)]
pub struct SiswaFilter {
    kelas_id: Option<String>,
    tahun: Option<String>,
}

impl SiswaFilter {
    fn kelas_id(&self) -> Option<i64> {
        self.kelas_id
            .as_deref()
            .and_then(|value| value.trim().parse::<i64>().ok())
    }

    fn tahun(&self) -> String {
        self.tahun
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string)
            .unwrap_or_else(data_tahun)
    }
}

#[derive(Serialize)]
struct SiswaFormData {
    nis: String,
    nama: String,
    kelas_id: Option<i64>,
    jenis_kelamin: String,
    kelas: Vec<KelasOption>,
    errors: HashMap<String, String>,
    action: String,
    page_title: String,
    submit_label: String,
}

#[derive(Deserialize)]
pub struct SiswaForm {
    nis: String,
    nama: String,
    kelas_id: String,
    jenis_kelamin: String,
}

#[derive(FromRow)]
struct SiswaFormRow {
    id: i64,
    nis: String,
    nama: String,
    kelas_id: i64,
    jenis_kelamin: Option<String>,
}

pub async fn data_siswa_index(
    ctx: PageContext,
    Query(filter): Query<SiswaFilter>,
    Extension(db): Extension<MySqlPool>,
) -> Html<String> {
    let kelas_id = filter.kelas_id();
    let tahun = filter.tahun();
    let mut list_tahun = load_tahun(&db).await;
    if !list_tahun.contains(&tahun) {
        list_tahun.push(tahun.clone());
        list_tahun.sort_by(|left, right| right.cmp(left));
    }
    render(
        &ctx,
        "guru/data_siswa/index.html",
        "Data Siswa",
        SiswaListData {
            kelas: load_kelas(&db).await,
            kelas_id,
            tahun,
            list_tahun,
        },
    )
}

pub async fn data_siswa_table(
    ctx: PageContext,
    Htmx(is_htmx): Htmx,
    Query(filter): Query<SiswaFilter>,
    Extension(db): Extension<MySqlPool>,
) -> axum::response::Response {
    if !is_htmx {
        return Redirect::to("/data-siswa").into_response();
    }

    let tahun = filter.tahun();
    let rows = fetch_siswa(&db, &tahun, filter.kelas_id()).await;
    let mut tera_ctx = Context::new();
    tera_ctx.insert("rows", &rows);
    Html(
        ctx.tera
            .render("guru/data_siswa/_table.html", &tera_ctx)
            .unwrap(),
    )
    .into_response()
}

pub async fn data_siswa_create(
    ctx: PageContext,
    Extension(db): Extension<MySqlPool>,
) -> Html<String> {
    render_form_page(
        &ctx,
        create_form_data(load_kelas(&db).await, HashMap::new()),
    )
}

pub async fn data_siswa_edit(
    ctx: PageContext,
    Path(id): Path<i64>,
    Extension(db): Extension<MySqlPool>,
) -> axum::response::Response {
    match find_siswa(&db, id).await {
        Ok(Some(siswa)) => render_form_page(
            &ctx,
            edit_form_data(siswa, load_kelas(&db).await, HashMap::new()),
        )
        .into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, "Data siswa tidak ditemukan.").into_response(),
        Err(error) => {
            eprintln!("ERROR data_siswa_edit: {error:?}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Gagal mengambil data siswa.",
            )
                .into_response()
        }
    }
}

pub async fn data_siswa_store(
    ctx: PageContext,
    Htmx(is_htmx): Htmx,
    Extension(db): Extension<MySqlPool>,
    Form(form): Form<SiswaForm>,
) -> axum::response::Response {
    let kelas = load_kelas(&db).await;
    let (nis, nama, kelas_id, jenis_kelamin, mut errors) = normalize_and_validate(form, &kelas);
    if errors.is_empty() && nis_exists(&db, &nis, None).await {
        errors.insert("nis".into(), "NIS sudah digunakan.".into());
    }
    if !errors.is_empty() {
        return form_error_response(
            &ctx,
            is_htmx,
            create_form_data_from_values(nis, nama, kelas_id, jenis_kelamin, kelas, errors),
        );
    }

    let password = match hash(DEFAULT_PASSWORD, DEFAULT_COST) {
        Ok(password) => password,
        Err(error) => {
            eprintln!("ERROR data_siswa password hash: {error:?}");
            return error_response(is_htmx, "Gagal menyiapkan password siswa.");
        }
    };

    match create_siswa(
        &db,
        &nis,
        &nama,
        kelas_id.unwrap(),
        &jenis_kelamin,
        &password,
    )
    .await
    {
        Ok(()) => success_response(is_htmx, "Data siswa berhasil ditambahkan."),
        Err(error) => {
            eprintln!("ERROR data_siswa_store: {error:?}");
            let message = if error.to_string().contains("Duplicate") {
                "NIS sudah digunakan."
            } else {
                "Data siswa gagal disimpan. Silakan coba lagi."
            };
            errors.insert("nis".into(), message.into());
            form_error_response(
                &ctx,
                is_htmx,
                create_form_data_from_values(nis, nama, kelas_id, jenis_kelamin, kelas, errors),
            )
        }
    }
}

pub async fn data_siswa_update(
    ctx: PageContext,
    Htmx(is_htmx): Htmx,
    Path(id): Path<i64>,
    Extension(db): Extension<MySqlPool>,
    Form(form): Form<SiswaForm>,
) -> axum::response::Response {
    let Some(existing) = (match find_siswa(&db, id).await {
        Ok(row) => row,
        Err(error) => {
            eprintln!("ERROR data_siswa_update lookup: {error:?}");
            return error_response(is_htmx, "Gagal mengambil data siswa.");
        }
    }) else {
        return (StatusCode::NOT_FOUND, "Data siswa tidak ditemukan.").into_response();
    };

    let kelas = load_kelas(&db).await;
    let (nis, nama, kelas_id, jenis_kelamin, mut errors) = normalize_and_validate(form, &kelas);
    if errors.is_empty() && nis_exists(&db, &nis, Some(id)).await {
        errors.insert("nis".into(), "NIS sudah digunakan.".into());
    }
    if !errors.is_empty() {
        return form_error_response(
            &ctx,
            is_htmx,
            edit_form_data_from_values(id, nis, nama, kelas_id, jenis_kelamin, kelas, errors),
        );
    }

    match update_siswa(
        &db,
        id,
        &existing.nis,
        &nis,
        &nama,
        kelas_id.unwrap(),
        &jenis_kelamin,
    )
    .await
    {
        Ok(()) => success_response(is_htmx, "Data siswa berhasil diperbarui."),
        Err(error) => {
            eprintln!("ERROR data_siswa_update: {error:?}");
            errors.insert(
                "nis".into(),
                "Data siswa gagal diperbarui. Pastikan NIS baru belum digunakan data ujian lain."
                    .into(),
            );
            form_error_response(
                &ctx,
                is_htmx,
                edit_form_data_from_values(id, nis, nama, kelas_id, jenis_kelamin, kelas, errors),
            )
        }
    }
}

pub async fn data_siswa_destroy(
    Htmx(is_htmx): Htmx,
    Path(id): Path<i64>,
    Extension(db): Extension<MySqlPool>,
) -> axum::response::Response {
    match delete_siswa(&db, id).await {
        Ok(true) => success_response(is_htmx, "Data siswa berhasil dihapus."),
        Ok(false) => (StatusCode::NOT_FOUND, "Data siswa tidak ditemukan.").into_response(),
        Err(error) => {
            eprintln!("ERROR data_siswa_destroy: {error:?}");
            error_response(is_htmx, "Data siswa gagal dihapus. Silakan coba lagi.")
        }
    }
}

async fn create_siswa(
    db: &MySqlPool,
    nis: &str,
    nama: &str,
    kelas_id: i64,
    jenis_kelamin: &str,
    password: &str,
) -> Result<(), sqlx::Error> {
    let mut tx = db.begin().await?;
    let tingkat = kelas_tingkat(&mut tx, kelas_id).await?;
    sqlx::query(
        "INSERT INTO user_siswa (name, nis, jenis_kelamin, password, created_at, updated_at) VALUES (?, ?, ?, ?, NOW(), NOW())",
    )
    .bind(nama)
    .bind(nis)
    .bind(jenis_kelamin)
    .bind(password)
    .execute(&mut *tx)
    .await?;
    sqlx::query("INSERT INTO siswas (nis, kelas_id, tingkat, tahun, created_at, updated_at) VALUES (?, ?, ?, ?, NOW(), NOW())")
        .bind(nis)
        .bind(kelas_id)
        .bind(tingkat)
        .bind(data_tahun())
        .execute(&mut *tx)
        .await?;
    tx.commit().await
}

async fn update_siswa(
    db: &MySqlPool,
    id: i64,
    old_nis: &str,
    nis: &str,
    nama: &str,
    kelas_id: i64,
    jenis_kelamin: &str,
) -> Result<(), sqlx::Error> {
    let mut tx = db.begin().await?;
    let tingkat = kelas_tingkat(&mut tx, kelas_id).await?;

    // Semua perubahan NIS dilakukan dalam transaksi agar data ujian tidak terpisah dari akun siswa.
    if old_nis != nis {
        sqlx::query("UPDATE ujian_jawabans SET nis = ?, updated_at = NOW() WHERE nis = ?")
            .bind(nis)
            .bind(old_nis)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE ujian_pesertas SET nis = ?, updated_at = NOW() WHERE nis = ?")
            .bind(nis)
            .bind(old_nis)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE ujian_pengerjaans SET nis = ?, updated_at = NOW() WHERE nis = ?")
            .bind(nis)
            .bind(old_nis)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE hasil_nilais SET nis = ?, updated_at = NOW() WHERE nis = ?")
            .bind(nis)
            .bind(old_nis)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE absensis SET nis = ?, updated_at = NOW() WHERE nis = ?")
            .bind(nis)
            .bind(old_nis)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE biodatas SET nis = ? WHERE nis = ?")
            .bind(nis)
            .bind(old_nis)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE orang_tuas SET nis = ? WHERE nis = ?")
            .bind(nis)
            .bind(old_nis)
            .execute(&mut *tx)
            .await?;
    }
    sqlx::query(
        "UPDATE user_siswa SET name = ?, nis = ?, jenis_kelamin = ?, updated_at = NOW() WHERE id = ?",
    )
    .bind(nama)
    .bind(nis)
    .bind(jenis_kelamin)
    .bind(id)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE siswas SET nis = ?, updated_at = NOW() WHERE nis = ?")
        .bind(nis)
        .bind(old_nis)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE siswas SET kelas_id = ?, tingkat = ?, updated_at = NOW() WHERE nis = ? AND tahun = ?")
        .bind(kelas_id)
        .bind(tingkat)
        .bind(nis)
        .bind(data_tahun())
        .execute(&mut *tx)
        .await?;
    tx.commit().await
}

async fn delete_siswa(db: &MySqlPool, id: i64) -> Result<bool, sqlx::Error> {
    let mut tx = db.begin().await?;
    let nis = sqlx::query_scalar::<_, String>(
        "SELECT CAST(nis AS CHAR) FROM user_siswa WHERE id = ? FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(nis) = nis else {
        tx.rollback().await?;
        return Ok(false);
    };

    // Penghapusan tidak memeriksa tabel lain; riwayat ujian tetap dibiarkan sebagai data historis.
    sqlx::query("DELETE FROM siswas WHERE nis = ?")
        .bind(&nis)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM user_siswa WHERE id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(true)
}

async fn kelas_tingkat(tx: &mut Transaction<'_, MySql>, kelas_id: i64) -> Result<i32, sqlx::Error> {
    sqlx::query_scalar::<_, i32>(
        "SELECT CAST(tingkat AS SIGNED) FROM kelas WHERE id = ? FOR UPDATE",
    )
    .bind(kelas_id)
    .fetch_one(&mut **tx)
    .await
}

async fn fetch_siswa(db: &MySqlPool, tahun: &str, kelas_id: Option<i64>) -> Vec<SiswaRow> {
    sqlx::query_as::<_, SiswaRow>(
        "SELECT CAST(u.id AS SIGNED) AS id, CAST(u.nis AS CHAR) AS nis, CAST(u.name AS CHAR) AS nama, CAST(k.nama AS CHAR) AS kelas FROM siswas s INNER JOIN user_siswa u ON u.nis = s.nis INNER JOIN kelas k ON k.id = s.kelas_id WHERE s.tahun = ? AND (? IS NULL OR s.kelas_id = ?) ORDER BY k.tingkat, k.nama, u.name, u.nis",
    ).bind(tahun).bind(kelas_id).bind(kelas_id).fetch_all(db).await.unwrap_or_else(|error| { eprintln!("ERROR fetch_siswa: {error:?}"); Vec::new() })
}

async fn load_tahun(db: &MySqlPool) -> Vec<String> {
    sqlx::query_scalar::<_, String>(
        "SELECT DISTINCT CAST(tahun AS CHAR) FROM siswas WHERE tahun IS NOT NULL AND tahun <> '' ORDER BY tahun DESC",
    )
    .fetch_all(db)
    .await
    .unwrap_or_else(|error| {
        eprintln!("ERROR load tahun siswa: {error:?}");
        Vec::new()
    })
}

async fn find_siswa(db: &MySqlPool, id: i64) -> Result<Option<SiswaFormRow>, sqlx::Error> {
    sqlx::query_as::<_, SiswaFormRow>(
        "SELECT CAST(u.id AS SIGNED) AS id, CAST(u.nis AS CHAR) AS nis, CAST(u.name AS CHAR) AS nama, CAST(s.kelas_id AS SIGNED) AS kelas_id, CAST(u.jenis_kelamin AS CHAR) AS jenis_kelamin FROM user_siswa u INNER JOIN siswas s ON s.nis = u.nis WHERE u.id = ? AND s.tahun = ? LIMIT 1",
    ).bind(id).bind(data_tahun()).fetch_optional(db).await
}

async fn load_kelas(db: &MySqlPool) -> Vec<KelasOption> {
    sqlx::query_as::<_, KelasOption>("SELECT CAST(id AS SIGNED) AS id, CAST(nama AS CHAR) AS nama, CAST(tingkat AS CHAR) AS tingkat FROM kelas ORDER BY CAST(tingkat AS UNSIGNED), nama")
        .fetch_all(db).await.unwrap_or_else(|error| { eprintln!("ERROR load kelas siswa: {error:?}"); Vec::new() })
}

async fn nis_exists(db: &MySqlPool, nis: &str, except_id: Option<i64>) -> bool {
    let query = if except_id.is_some() {
        "SELECT COUNT(*) FROM user_siswa WHERE nis = ? AND id <> ?"
    } else {
        "SELECT COUNT(*) FROM user_siswa WHERE nis = ?"
    };
    let mut statement = sqlx::query_scalar::<_, i64>(query).bind(nis);
    if let Some(id) = except_id {
        statement = statement.bind(id);
    }
    statement.fetch_one(db).await.unwrap_or(0) > 0
}

fn normalize_and_validate(
    form: SiswaForm,
    kelas: &[KelasOption],
) -> (String, String, Option<i64>, String, HashMap<String, String>) {
    let nis = form.nis.trim().to_string();
    let nama = form.nama.trim().to_string();
    let jenis_kelamin = form.jenis_kelamin.trim().to_uppercase();
    let kelas_id = form.kelas_id.trim().parse::<i64>().ok();
    let mut errors = HashMap::new();
    match nis.parse::<u32>() {
        Ok(value)
            if nis.chars().all(|character| character.is_ascii_digit())
                && value > 0
                && value <= MAX_NIS => {}
        _ => {
            errors.insert(
                "nis".into(),
                format!("NIS harus berupa angka 1 sampai {MAX_NIS}."),
            );
        }
    }
    if nama.is_empty() {
        errors.insert("nama".into(), "Nama siswa wajib diisi.".into());
    } else if nama.chars().count() > 255 {
        errors.insert("nama".into(), "Nama siswa maksimal 255 karakter.".into());
    }
    if !matches!(jenis_kelamin.as_str(), "L" | "P") {
        errors.insert(
            "jenis_kelamin".into(),
            "Jenis kelamin wajib dipilih.".into(),
        );
    }
    if kelas_id.is_none_or(|id| !kelas.iter().any(|item| item.id == id)) {
        errors.insert("kelas_id".into(), "Kelas wajib dipilih.".into());
    }
    (nis, nama, kelas_id, jenis_kelamin, errors)
}

fn create_form_data(kelas: Vec<KelasOption>, errors: HashMap<String, String>) -> SiswaFormData {
    create_form_data_from_values(
        String::new(),
        String::new(),
        None,
        String::new(),
        kelas,
        errors,
    )
}
fn create_form_data_from_values(
    nis: String,
    nama: String,
    kelas_id: Option<i64>,
    jenis_kelamin: String,
    kelas: Vec<KelasOption>,
    errors: HashMap<String, String>,
) -> SiswaFormData {
    SiswaFormData {
        nis,
        nama,
        kelas_id,
        jenis_kelamin,
        kelas,
        errors,
        action: "/data-siswa".into(),
        page_title: "Tambah Data Siswa".into(),
        submit_label: "Simpan".into(),
    }
}
fn edit_form_data(
    siswa: SiswaFormRow,
    kelas: Vec<KelasOption>,
    errors: HashMap<String, String>,
) -> SiswaFormData {
    edit_form_data_from_values(
        siswa.id,
        siswa.nis,
        siswa.nama,
        Some(siswa.kelas_id),
        siswa.jenis_kelamin.unwrap_or_default(),
        kelas,
        errors,
    )
}
fn edit_form_data_from_values(
    id: i64,
    nis: String,
    nama: String,
    kelas_id: Option<i64>,
    jenis_kelamin: String,
    kelas: Vec<KelasOption>,
    errors: HashMap<String, String>,
) -> SiswaFormData {
    SiswaFormData {
        nis,
        nama,
        kelas_id,
        jenis_kelamin,
        kelas,
        errors,
        action: format!("/data-siswa/{id}"),
        page_title: "Edit Data Siswa".into(),
        submit_label: "Simpan Perubahan".into(),
    }
}
fn render_form_page(ctx: &PageContext, data: SiswaFormData) -> Html<String> {
    let title = data.page_title.clone();
    render(ctx, "guru/data_siswa/form.html", &title, data)
}
fn render_form_partial(ctx: &PageContext, data: &SiswaFormData) -> String {
    let mut tera_ctx = tera::Context::new();
    tera_ctx.insert("data", data);
    ctx.tera
        .render("guru/data_siswa/_form.html", &tera_ctx)
        .unwrap()
}
fn form_error_response(
    ctx: &PageContext,
    is_htmx: bool,
    data: SiswaFormData,
) -> axum::response::Response {
    if is_htmx {
        return (StatusCode::OK, Html(render_form_partial(ctx, &data))).into_response();
    }
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        render_form_page(ctx, data),
    )
        .into_response()
}
fn flash_headers(message: &str, kind: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        "HX-Trigger",
        format!(r#"{{"flash":{{"message":"{message}","type":"{kind}"}}}}"#)
            .parse()
            .unwrap(),
    );
    headers
}
fn success_response(is_htmx: bool, message: &str) -> axum::response::Response {
    if is_htmx {
        let mut headers = flash_headers(message, "success");
        headers.insert("HX-Redirect", "/data-siswa".parse().unwrap());
        return (headers, Html(String::new())).into_response();
    }
    Redirect::to("/data-siswa").into_response()
}
fn error_response(is_htmx: bool, message: &str) -> axum::response::Response {
    if is_htmx {
        return (flash_headers(message, "error"), Html(String::new())).into_response();
    }
    (StatusCode::CONFLICT, message.to_string()).into_response()
}
