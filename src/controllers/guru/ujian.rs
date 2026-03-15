use aws_sdk_s3::primitives::ByteStream;
use axum::{
    Json,
    extract::{Form, Multipart, Path, Query},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect},
};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, MySqlPool};
use tera::Context;
use uuid::Uuid;

use crate::config::s3::S3State;
use crate::utils::{page_context::PageContext, render::render, tahun::data_tahun};

use super::absensi_kelas::Htmx;

#[derive(Serialize)]
struct UjianListData {
    tahun: String,
    list_ujian: Vec<UjianRow>,
}

#[derive(Serialize, FromRow)]
struct UjianRow {
    id: i64,
    user_id: Option<u64>,
    title: String,
    description: Option<String>,
    tanggal: NaiveDate,
    waktu_menit: i32,
    total_soal: i32,
    is_active: i8,
    jurusan: String,
    mata_pelajaran: Option<String>,
    created_at: Option<String>,
}

#[derive(Deserialize)]
pub struct UjianFilter {
    pub tahun: Option<String>,
    pub search: Option<String>,
}

#[derive(Deserialize)]
pub struct ProgressFilter {
    pub ujian_id: Option<i64>,
    #[serde(default, deserialize_with = "empty_string_as_none_i64")]
    pub mata_pelajaran_id: Option<i64>,
    pub tahun: Option<String>,
}

#[derive(Deserialize)]
pub struct UraianFilter {
    #[serde(default, deserialize_with = "empty_string_as_none_i64")]
    pub kelas_id: Option<i64>,
    #[serde(default, deserialize_with = "empty_string_as_none_i64")]
    pub mata_pelajaran_id: Option<i64>,
    pub tahun: Option<String>,
}

#[derive(Deserialize)]
pub struct StatusPesertaFilter {
    pub ujian_id: Option<i64>,
    pub tahun: Option<String>,
}

#[derive(Deserialize, Clone)]
pub struct NilaiFilter {
    #[serde(default, deserialize_with = "empty_string_as_none_i64")]
    pub kelas_id: Option<i64>,
    #[serde(default, deserialize_with = "empty_string_as_none_i64")]
    pub mata_pelajaran_id: Option<i64>,
    pub tahun: Option<String>,
}

#[derive(Clone, Serialize, FromRow)]
struct UjianOption {
    id: i64,
    mata_pelajaran_id: Option<i64>,
    title: String,
    mata_pelajaran: Option<String>,
    is_active: i8,
}

#[derive(Serialize, FromRow)]
struct UraianJawabanRow {
    jawaban_id: i64,
    ujian_id: i64,
    ujian_title: Option<String>,
    kelas: Option<String>,
    nis: String,
    nama: Option<String>,
    soal_id: i64,
    pertanyaan: Option<String>,
    jawaban_uraian: Option<String>,
    nilai_uraian: i32,
    status_uraian: Option<String>,
    bobot_nilai: Option<f64>,
}

#[derive(Deserialize)]
pub struct UraianScoreForm {
    pub jawaban_id: i64,
    pub ujian_id: i64,
    pub nis: String,
    pub nilai_uraian: i32,
}

#[derive(Serialize, FromRow)]
struct NilaiKelasRow {
    nis: String,
    nama: Option<String>,
    kelas: Option<String>,
    total_benar: Option<f64>,
    total_salah: Option<f64>,
    nilai_pg: Option<f64>,
    nilai_uraian: Option<f64>,
    total_nilai: Option<f64>,
}

#[derive(Serialize)]
struct CreateUjianData {
    tahun: String,
    title: String,
    description: String,
    tanggal: String,
    waktu_menit: i32,
    mata_pelajaran_id: i64,
    jurusan: String,
    mapel_options: Vec<MapelOption>,
    errors: std::collections::HashMap<String, String>,
}

#[derive(Serialize, FromRow)]
struct MapelOption {
    id: i64,
    nama: String,
}

#[derive(Deserialize)]
pub struct CreateUjianForm {
    pub tahun: String,
    pub mata_pelajaran_id: i64,
    pub jurusan: String,
    pub title: String,
    pub description: Option<String>,
    pub tanggal: String,
    pub waktu_menit: i32,
}

#[derive(Serialize)]
struct UjianDetailData {
    ujian: Option<UjianDetailRow>,
    list_soal: Vec<SoalRow>,
    list_token: Vec<TokenRow>,
    list_hasil: Vec<HasilRow>,
    can_add_soal: bool,
}

#[derive(Serialize, FromRow)]
struct UjianDetailRow {
    id: i64,
    user_id: Option<u64>,
    title: String,
    description: Option<String>,
    tanggal: NaiveDate,
    waktu_menit: i32,
    total_soal: i32,
    is_active: i8,
    jurusan: String,
    mata_pelajaran: Option<String>,
}

#[derive(Serialize, FromRow)]
struct SoalRow {
    id: i64,
    pertanyaan: Option<String>,
    opsi_a: Option<String>,
    opsi_b: Option<String>,
    opsi_c: Option<String>,
    opsi_d: Option<String>,
    opsi_e: Option<String>,
    kunci_jawaban: Option<String>,
    bobot_nilai: Option<f64>,
    kategori: Option<String>,
    urutan: Option<i32>,
}

#[derive(Serialize, FromRow)]
struct TokenRow {
    id: i64,
    token: String,
    is_active: i8,
    expired_at: Option<String>,
    created_at: Option<String>,
}

#[derive(Serialize, FromRow)]
struct HasilRow {
    nama: String,
    nis: Option<String>,
    status: String,
    last_nomor: i32,
    total_nilai: Option<f64>,
    submitted_at: Option<String>,
}

#[derive(Serialize)]
struct CreateSoalData {
    ujian_id: i64,
    ujian_title: String,
    pertanyaan: String,
    opsi_a: String,
    opsi_b: String,
    opsi_c: String,
    opsi_d: String,
    opsi_e: String,
    kunci_jawaban: String,
    bobot_nilai: f64,
    kategori: String,
}

#[derive(Deserialize)]
pub struct CreateSoalForm {
    pub pertanyaan: String,
    pub opsi_a: String,
    pub opsi_b: String,
    pub opsi_c: String,
    pub opsi_d: String,
    pub opsi_e: String,
    pub kunci_jawaban: Option<String>,
    pub bobot_nilai: Option<f64>,
    pub kategori: Option<String>,
}

#[derive(Deserialize)]
pub struct ToggleAktifForm {
    pub active: i8,
}

fn flash_success(message: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        "HX-Trigger",
        format!(
            r#"{{"flash":{{"message":"{}","type":"success"}}}}"#,
            message
        )
        .parse()
        .unwrap(),
    );
    headers
}

fn flash_error(message: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        "HX-Trigger",
        format!(r#"{{"flash":{{"message":"{}","type":"error"}}}}"#, message)
            .parse()
            .unwrap(),
    );
    headers
}

fn valid_jurusan(v: &str) -> bool {
    matches!(v, "UMUM" | "PBS" | "TKR" | "TKJ")
}

fn has_visible_content(html: &str) -> bool {
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ => {
                if !in_tag && !c.is_whitespace() {
                    return true;
                }
            }
        }
    }
    html.contains("<img")
}

fn contains_data_image(html: &str) -> bool {
    html.contains("data:image")
}

async fn fetch_mapel_options(db: &MySqlPool) -> Vec<MapelOption> {
    sqlx::query_as::<_, MapelOption>(
        "SELECT CAST(id AS SIGNED) as id, nama FROM mata_pelajarans ORDER BY nama ASC",
    )
    .fetch_all(db)
    .await
    .unwrap_or_default()
}

async fn fetch_ujian_options(db: &MySqlPool, tahun: &str) -> Vec<UjianOption> {
    let pattern = format!("%{}%", tahun.trim());
    eprintln!(
        "DEBUG fetch_ujian_options: bound tahun pattern='{}'",
        pattern
    );
    match sqlx::query_as::<_, UjianOption>(
        r#"
        SELECT
            CAST(u.id AS SIGNED) as id,
            CAST(u.mata_pelajaran_id AS SIGNED) as mata_pelajaran_id,
            u.title,
            mp.nama as mata_pelajaran,
            u.is_active
        FROM ujians u
        LEFT JOIN mata_pelajarans mp ON mp.id = u.mata_pelajaran_id
        WHERE u.tahun LIKE ?
        ORDER BY u.created_at DESC
        LIMIT 200
        "#,
    )
    .bind(pattern)
    .fetch_all(db)
    .await
    {
        Ok(rows) => {
            eprintln!(
                "DEBUG fetch_ujian_options: fetched {} ujian rows",
                rows.len()
            );
            rows
        }
        Err(e) => {
            eprintln!(
                "ERROR fetch_ujian_options: query failed for tahun='{}': {:?}",
                tahun, e
            );
            Vec::new()
        }
    }
}

pub async fn ujian_index(
    ctx: PageContext,
    axum::Extension(_db): axum::Extension<MySqlPool>,
) -> Html<String> {
    let data = UjianListData {
        tahun: data_tahun(),
        list_ujian: Vec::new(),
    };

    render(&ctx, "guru/ujian/index.html", "Daftar Ujian", data)
}

pub async fn ujian_table(
    ctx: PageContext,
    Htmx(is_htmx): Htmx,
    Query(filter): Query<UjianFilter>,
    axum::Extension(db): axum::Extension<MySqlPool>,
) -> axum::response::Response {
    let tahun = filter.tahun.clone().unwrap_or_else(data_tahun);
    let search = filter.search.clone().unwrap_or_default();

    if !is_htmx {
        let url = format!("/ujian?tahun={}&search={}", tahun, search);
        return Redirect::to(&url).into_response();
    }

    let search_like = format!("%{}%", search);

    let rows: Vec<UjianRow> = sqlx::query_as::<_, UjianRow>(
        r#"
        SELECT
            u.id,
            u.user_id,
            u.title,
            u.description,
            u.tanggal,
            u.waktu_menit,
            COALESCE(u.total_soal, 0) as total_soal,
            u.is_active,
            u.jurusan,
            mp.nama as mata_pelajaran,
            DATE_FORMAT(u.created_at, '%Y-%m-%d %H:%i') as created_at
        FROM ujians u
        JOIN mata_pelajarans mp ON mp.id = u.mata_pelajaran_id
        WHERE u.tahun = ?
          AND (u.title LIKE ? OR u.description LIKE ? OR mp.nama LIKE ?)
        ORDER BY u.created_at DESC
        LIMIT 100
        "#,
    )
    .bind(&tahun)
    .bind(&search_like)
    .bind(&search_like)
    .bind(&search_like)
    .fetch_all(&db)
    .await
    .unwrap_or_default();

    let mut tera_ctx = Context::new();
    tera_ctx.insert("list_ujian", &rows);
    tera_ctx.insert("filter_tahun", &tahun);
    tera_ctx.insert("filter_search", &search);
    tera_ctx.insert("current_user_id", &ctx.user.id);

    let rendered = ctx
        .tera
        .render("guru/_ujian_table.html", &tera_ctx)
        .unwrap();

    Html(rendered).into_response()
}

pub async fn ujian_create(
    ctx: PageContext,
    axum::Extension(db): axum::Extension<MySqlPool>,
) -> Html<String> {
    let data = CreateUjianData {
        tahun: data_tahun(),
        title: String::new(),
        description: String::new(),
        tanggal: chrono::Local::now().format("%Y-%m-%d").to_string(),
        waktu_menit: 60,
        mata_pelajaran_id: 0,
        jurusan: "UMUM".to_string(),
        mapel_options: fetch_mapel_options(&db).await,
        errors: std::collections::HashMap::new(),
    };

    render(&ctx, "guru/ujian/create.html", "Buat Ujian Baru", data)
}

pub async fn ujian_store(
    ctx: PageContext,
    Htmx(is_htmx): Htmx,
    axum::Extension(db): axum::Extension<MySqlPool>,
    Form(form): Form<CreateUjianForm>,
) -> axum::response::Response {
    if !is_htmx {
        return Redirect::to("/ujian/create").into_response();
    }

    let mut errors = std::collections::HashMap::<String, String>::new();

    if form.title.trim().is_empty() {
        errors.insert("title".to_string(), "Judul ujian wajib diisi".to_string());
    }
    if form.tahun.trim().is_empty() {
        errors.insert(
            "tahun".to_string(),
            "Tahun ajaran wajib dipilih".to_string(),
        );
    }
    if form.mata_pelajaran_id <= 0 {
        errors.insert(
            "mata_pelajaran_id".to_string(),
            "Mata pelajaran wajib dipilih".to_string(),
        );
    }
    let jurusan = form.jurusan.trim().to_uppercase();
    if !valid_jurusan(&jurusan) {
        errors.insert("jurusan".to_string(), "Jurusan tidak valid".to_string());
    }

    if !errors.is_empty() {
        let data = CreateUjianData {
            tahun: form.tahun.clone(),
            title: form.title,
            description: form.description.unwrap_or_default(),
            tanggal: form.tanggal,
            waktu_menit: form.waktu_menit,
            mata_pelajaran_id: form.mata_pelajaran_id,
            jurusan,
            mapel_options: fetch_mapel_options(&db).await,
            errors,
        };
        let rendered = render(&ctx, "guru/ujian/create.html", "Buat Ujian Baru", data);
        let mut headers = HeaderMap::new();
        headers.insert("HX-Retarget", "#ujian-form".parse().unwrap());
        headers.insert("HX-Reswap", "innerHTML".parse().unwrap());
        return (headers, Html(rendered)).into_response();
    }

    let result = sqlx::query(
        r#"
        INSERT INTO ujians
            (mata_pelajaran_id, tahun, jurusan, title, description, tanggal, waktu_menit, is_active, user_id, created_by, created_at, updated_at)
        VALUES
            (?, ?, ?, ?, ?, ?, ?, FALSE, ?, ?, NOW(), NOW())
        "#,
    )
    .bind(form.mata_pelajaran_id)
    .bind(&form.tahun)
    .bind(&jurusan)
    .bind(&form.title)
    .bind(&form.description)
    .bind(&form.tanggal)
    .bind(form.waktu_menit)
    .bind(ctx.user.id)
    .bind(ctx.user.id)
    .execute(&db)
    .await;

    match result {
        Ok(_) => {
            let mut headers = flash_success("Ujian berhasil dibuat!");
            headers.insert("HX-Redirect", "/ujian".parse().unwrap());
            (headers, Html(String::new())).into_response()
        }
        Err(e) => {
            eprintln!("Error creating ujian: {:?}", e);
            let mut headers = flash_error("Gagal membuat ujian!");
            headers.insert("HX-Redirect", "/ujian".parse().unwrap());
            (headers, Html(String::new())).into_response()
        }
    }
}

pub async fn ujian_show(
    ctx: PageContext,
    Path(ujian_id): Path<i64>,
    axum::Extension(db): axum::Extension<MySqlPool>,
) -> axum::response::Response {
    // Show only ujian details and soal list; no student-related updates here.
    let ujian: Option<UjianDetailRow> = sqlx::query_as::<_, UjianDetailRow>(
        r#"
        SELECT
            u.id,
            u.user_id,
            u.title,
            u.description,
            u.tanggal,
            u.waktu_menit,
            COALESCE(u.total_soal, 0) as total_soal,
            u.is_active,
            u.jurusan,
            mp.nama as mata_pelajaran
        FROM ujians u
        JOIN mata_pelajarans mp ON mp.id = u.mata_pelajaran_id
        WHERE u.id = ?
        "#,
    )
    .bind(ujian_id)
    .fetch_optional(&db)
    .await
    .unwrap_or(None);

    if ujian.is_none() {
        return Redirect::to("/ujian").into_response();
    }

    // Fetch soal rows; cast ids to signed to match Rust i64, and surface DB errors
    let list_soal: Vec<SoalRow> = match sqlx::query_as::<_, SoalRow>(
        r#"
        SELECT
            CAST(s.id AS SIGNED) as id,
            s.pertanyaan,
            s.opsi_a,
            s.opsi_b,
            s.opsi_c,
            s.opsi_d,
            s.opsi_e,
            s.kunci_jawaban,
            CAST(s.bobot_nilai AS DOUBLE) as bobot_nilai,
            s.kategori,
            CAST(us.urutan AS SIGNED) as urutan
        FROM soals s
        JOIN ujian_soals us ON us.soal_id = s.id
        WHERE us.ujian_id = ?
        ORDER BY us.urutan ASC
        "#,
    )
    .bind(ujian_id)
    .fetch_all(&db)
    .await
    {
        Ok(rows) => rows,
        Err(e) => {
            eprintln!(
                "ERROR ujian_show: failed to fetch soals for ujian_id={}: {:?}",
                ujian_id, e
            );
            Vec::new()
        }
    };

    // Debug: log how many soal we retrieved and their IDs
    let soal_ids: Vec<i64> = list_soal.iter().map(|s| s.id).collect();
    eprintln!(
        "DEBUG ujian_show: ujian_id={}, fetched_soal_count={}, ids={:?}",
        ujian_id,
        list_soal.len(),
        soal_ids
    );

    let can_add = ujian
        .as_ref()
        .and_then(|u| u.user_id)
        .map(|owner_id| owner_id == ctx.user.id)
        .unwrap_or(false);

    let data = UjianDetailData {
        ujian,
        list_soal,
        list_token: Vec::new(),
        list_hasil: Vec::new(),
        can_add_soal: can_add,
    };

    let html = render(&ctx, "guru/ujian/show.html", "Detail Ujian", data);
    Html(html.0).into_response()
}

mod nilai;
mod progress;
mod status_peserta;
mod uraian;

pub use nilai::{nilai_kelas_mapel_export, nilai_kelas_mapel_page, nilai_kelas_mapel_table};
pub use progress::{ujian_progress, ujian_progress_active_panel};
pub use status_peserta::{status_peserta_page, status_peserta_table, status_peserta_toggle};
pub use uraian::{ujian_uraian_review, ujian_uraian_score, ujian_uraian_table};

fn empty_string_as_none_i64<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt = Option::<String>::deserialize(deserializer)?;
    Ok(opt.and_then(|s| {
        let s = s.trim().to_string();
        if s.is_empty() {
            None
        } else {
            s.parse::<i64>().ok()
        }
    }))
}

pub async fn ujian_delete(
    Htmx(is_htmx): Htmx,
    Path(ujian_id): Path<i64>,
    axum::Extension(db): axum::Extension<MySqlPool>,
) -> axum::response::Response {
    if !is_htmx {
        return StatusCode::BAD_REQUEST.into_response();
    }

    let result = sqlx::query("DELETE FROM ujians WHERE id = ?")
        .bind(ujian_id)
        .execute(&db)
        .await;

    match result {
        Ok(_) => {
            let headers = flash_success("Ujian berhasil dihapus!");
            (headers, Html(String::new())).into_response()
        }
        Err(e) => {
            eprintln!("Error deleting ujian: {:?}", e);
            let headers = flash_error("Gagal menghapus ujian!");
            (headers, Html(String::new())).into_response()
        }
    }
}

pub async fn ujian_toggle_active(
    _ctx: PageContext,
    Htmx(is_htmx): Htmx,
    req_headers: HeaderMap,
    Path(ujian_id): Path<i64>,
    axum::Extension(db): axum::Extension<MySqlPool>,
    Form(form): Form<ToggleAktifForm>,
) -> axum::response::Response {
    if !is_htmx {
        return Redirect::to(&format!("/ujian/{}", ujian_id)).into_response();
    }

    if form.active == 1 {
        let tahun: Option<String> =
            sqlx::query_scalar("SELECT tahun FROM ujians WHERE id = ? LIMIT 1")
                .bind(ujian_id)
                .fetch_optional(&db)
                .await
                .ok()
                .flatten();
        if let Some(tahun) = tahun {
            let _ =
                sqlx::query("UPDATE ujians SET is_active = 0, updated_at = NOW() WHERE tahun = ?")
                    .bind(tahun)
                    .execute(&db)
                    .await;
        }
    }

    let result = sqlx::query("UPDATE ujians SET is_active = ?, updated_at = NOW() WHERE id = ?")
        .bind(form.active)
        .bind(ujian_id)
        .execute(&db)
        .await;

    match result {
        Ok(_) => {
            let mut headers = flash_success(if form.active == 1 {
                "Ujian diaktifkan. Ujian lain dinonaktifkan otomatis."
            } else {
                "Ujian dinonaktifkan."
            });
            let is_progress_page = req_headers
                .get("HX-Current-URL")
                .and_then(|v| v.to_str().ok())
                .map(|v| v.contains("/progress-ujian"))
                .unwrap_or(false);
            if !is_progress_page {
                headers.insert(
                    "HX-Redirect",
                    format!("/ujian/{}", ujian_id).parse().unwrap(),
                );
            } else {
                headers.insert(
                    "HX-Trigger-After-Settle",
                    "refresh-active,refresh-progress".parse().unwrap(),
                );
            }
            (headers, Html(String::new())).into_response()
        }
        Err(e) => {
            eprintln!("Error toggling ujian active: {:?}", e);
            let mut headers = flash_error("Gagal mengubah status ujian.");
            let is_progress_page = req_headers
                .get("HX-Current-URL")
                .and_then(|v| v.to_str().ok())
                .map(|v| v.contains("/progress-ujian"))
                .unwrap_or(false);
            if !is_progress_page {
                headers.insert(
                    "HX-Redirect",
                    format!("/ujian/{}", ujian_id).parse().unwrap(),
                );
            } else {
                headers.insert(
                    "HX-Trigger-After-Settle",
                    "refresh-active,refresh-progress".parse().unwrap(),
                );
            }
            (headers, Html(String::new())).into_response()
        }
    }
}

pub async fn ujian_generate_token(
    ctx: PageContext,
    Htmx(is_htmx): Htmx,
    req_headers: HeaderMap,
    Path(ujian_id): Path<i64>,
    axum::Extension(db): axum::Extension<MySqlPool>,
) -> axum::response::Response {
    if !is_htmx {
        return Redirect::to(&format!("/ujian/{}", ujian_id)).into_response();
    }

    let token = Uuid::new_v4()
        .simple()
        .to_string()
        .chars()
        .take(8)
        .collect::<String>()
        .to_uppercase();

    let result = sqlx::query(
        r#"
        INSERT INTO ujian_tokens (ujian_id, token, is_active, created_by, created_at)
        VALUES (?, ?, 1, ?, NOW())
        "#,
    )
    .bind(ujian_id)
    .bind(token)
    .bind(ctx.user.id)
    .execute(&db)
    .await;

    match result {
        Ok(_) => {
            let mut headers = flash_success("Token ujian berhasil dibuat.");
            let is_progress_page = req_headers
                .get("HX-Current-URL")
                .and_then(|v| v.to_str().ok())
                .map(|v| v.contains("/progress-ujian"))
                .unwrap_or(false);
            if !is_progress_page {
                headers.insert(
                    "HX-Redirect",
                    format!("/ujian/{}", ujian_id).parse().unwrap(),
                );
            } else {
                headers.insert(
                    "HX-Trigger-After-Settle",
                    "refresh-progress".parse().unwrap(),
                );
            }
            (headers, Html(String::new())).into_response()
        }
        Err(e) => {
            eprintln!("Error generating ujian token: {:?}", e);
            let mut headers = flash_error("Gagal membuat token ujian.");
            let is_progress_page = req_headers
                .get("HX-Current-URL")
                .and_then(|v| v.to_str().ok())
                .map(|v| v.contains("/progress-ujian"))
                .unwrap_or(false);
            if !is_progress_page {
                headers.insert(
                    "HX-Redirect",
                    format!("/ujian/{}", ujian_id).parse().unwrap(),
                );
            } else {
                headers.insert(
                    "HX-Trigger-After-Settle",
                    "refresh-progress".parse().unwrap(),
                );
            }
            (headers, Html(String::new())).into_response()
        }
    }
}

pub async fn ujian_toggle_token(
    Htmx(is_htmx): Htmx,
    req_headers: HeaderMap,
    Path((ujian_id, token_id)): Path<(i64, i64)>,
    axum::Extension(db): axum::Extension<MySqlPool>,
    Form(form): Form<ToggleAktifForm>,
) -> axum::response::Response {
    if !is_htmx {
        return Redirect::to(&format!("/ujian/{}", ujian_id)).into_response();
    }

    let result = sqlx::query("UPDATE ujian_tokens SET is_active = ? WHERE id = ? AND ujian_id = ?")
        .bind(form.active)
        .bind(token_id)
        .bind(ujian_id)
        .execute(&db)
        .await;

    match result {
        Ok(_) => {
            let mut headers = flash_success("Status token berhasil diperbarui.");
            let is_progress_page = req_headers
                .get("HX-Current-URL")
                .and_then(|v| v.to_str().ok())
                .map(|v| v.contains("/progress-ujian"))
                .unwrap_or(false);
            if !is_progress_page {
                headers.insert(
                    "HX-Redirect",
                    format!("/ujian/{}", ujian_id).parse().unwrap(),
                );
            } else {
                headers.insert(
                    "HX-Trigger-After-Settle",
                    "refresh-progress".parse().unwrap(),
                );
            }
            (headers, Html(String::new())).into_response()
        }
        Err(e) => {
            eprintln!("Error toggling token active: {:?}", e);
            let mut headers = flash_error("Gagal mengubah status token.");
            let is_progress_page = req_headers
                .get("HX-Current-URL")
                .and_then(|v| v.to_str().ok())
                .map(|v| v.contains("/progress-ujian"))
                .unwrap_or(false);
            if !is_progress_page {
                headers.insert(
                    "HX-Redirect",
                    format!("/ujian/{}", ujian_id).parse().unwrap(),
                );
            } else {
                headers.insert(
                    "HX-Trigger-After-Settle",
                    "refresh-progress".parse().unwrap(),
                );
            }
            (headers, Html(String::new())).into_response()
        }
    }
}

pub async fn ujian_delete_token(
    Htmx(is_htmx): Htmx,
    req_headers: HeaderMap,
    Path((ujian_id, token_id)): Path<(i64, i64)>,
    axum::Extension(db): axum::Extension<MySqlPool>,
) -> axum::response::Response {
    if !is_htmx {
        return StatusCode::BAD_REQUEST.into_response();
    }

    let result = sqlx::query("DELETE FROM ujian_tokens WHERE id = ? AND ujian_id = ?")
        .bind(token_id)
        .bind(ujian_id)
        .execute(&db)
        .await;

    match result {
        Ok(_) => {
            let mut headers = flash_success("Token berhasil dihapus.");
            let is_progress_page = req_headers
                .get("HX-Current-URL")
                .and_then(|v| v.to_str().ok())
                .map(|v| v.contains("/progress-ujian"))
                .unwrap_or(false);
            if is_progress_page {
                headers.insert(
                    "HX-Trigger-After-Settle",
                    "refresh-progress".parse().unwrap(),
                );
            }
            (headers, Html(String::new())).into_response()
        }
        Err(e) => {
            eprintln!("Error deleting token: {:?}", e);
            let headers = flash_error("Gagal menghapus token.");
            (headers, Html(String::new())).into_response()
        }
    }
}

pub async fn soal_create(
    ctx: PageContext,
    Htmx(is_htmx): Htmx,
    Path(ujian_id): Path<i64>,
    axum::Extension(db): axum::Extension<MySqlPool>,
) -> axum::response::Response {
    let ujian: Option<UjianDetailRow> = sqlx::query_as::<_, UjianDetailRow>(
        r#"
        SELECT
            u.id,
            u.user_id,
            u.title,
            u.description,
            u.tanggal,
            u.waktu_menit,
            COALESCE(u.total_soal, 0) as total_soal,
            u.is_active,
            u.jurusan,
            mp.nama as mata_pelajaran
        FROM ujians u
        JOIN mata_pelajarans mp ON mp.id = u.mata_pelajaran_id
        WHERE u.id = ?
        "#,
    )
    .bind(ujian_id)
    .fetch_optional(&db)
    .await
    .unwrap_or(None);

    if ujian.is_none() {
        return Redirect::to("/ujian").into_response();
    }

    let data = CreateSoalData {
        ujian_id,
        ujian_title: ujian.unwrap().title,
        pertanyaan: String::new(),
        opsi_a: String::new(),
        opsi_b: String::new(),
        opsi_c: String::new(),
        opsi_d: String::new(),
        opsi_e: String::new(),
        kunci_jawaban: "a".to_string(),
        bobot_nilai: 1.0,
        kategori: "Pilihan Ganda".to_string(),
    };

    if is_htmx {
        let html = render(&ctx, "guru/ujian/_soal_form.html", "Tambah Soal", data);
        Html(html.0).into_response()
    } else {
        let html = render(&ctx, "guru/ujian/soal_create.html", "Tambah Soal", data);
        Html(html.0).into_response()
    }
}

pub async fn soal_store(
    _ctx: PageContext,
    Htmx(is_htmx): Htmx,
    Path(ujian_id): Path<i64>,
    axum::Extension(db): axum::Extension<MySqlPool>,
    Form(form): Form<CreateSoalForm>,
) -> axum::response::Response {
    if !is_htmx {
        return Redirect::to(&format!("/ujian/{}/soal/create", ujian_id)).into_response();
    }

    if form.pertanyaan.trim().is_empty() {
        let mut headers = flash_error("Pertanyaan tidak boleh kosong!");
        headers.insert("HX-Retarget", "#soal-form-container".parse().unwrap());
        headers.insert("HX-Reswap", "none".parse().unwrap());
        return (headers, Html(String::new())).into_response();
    }

    if contains_data_image(&form.pertanyaan)
        || contains_data_image(&form.opsi_a)
        || contains_data_image(&form.opsi_b)
        || contains_data_image(&form.opsi_c)
        || contains_data_image(&form.opsi_d)
        || contains_data_image(&form.opsi_e)
    {
        let mut headers =
            flash_error("Gambar base64 tidak diperbolehkan. Gunakan tombol upload gambar.");
        headers.insert("HX-Retarget", "#soal-form-container".parse().unwrap());
        headers.insert("HX-Reswap", "none".parse().unwrap());
        return (headers, Html(String::new())).into_response();
    }

    let kategori = form
        .kategori
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("Pilihan Ganda");

    if kategori != "Uraian" {
        // For Pilihan Ganda and Kompleks require opsi content; for Benar/Salah we don't
        if kategori != "Benar/Salah" {
            if !has_visible_content(&form.opsi_a)
                || !has_visible_content(&form.opsi_b)
                || !has_visible_content(&form.opsi_c)
                || !has_visible_content(&form.opsi_d)
                || !has_visible_content(&form.opsi_e)
            {
                let mut headers =
                    flash_error("Opsi A, B, C, D, dan E wajib diisi (teks atau gambar).");
                headers.insert("HX-Retarget", "#soal-form-container".parse().unwrap());
                headers.insert("HX-Reswap", "none".parse().unwrap());
                return (headers, Html(String::new())).into_response();
            }
        }

        let Some(kunci_raw) = form.kunci_jawaban.as_deref() else {
            let mut headers = flash_error("Kunci jawaban wajib dipilih.");
            headers.insert("HX-Retarget", "#soal-form-container".parse().unwrap());
            headers.insert("HX-Reswap", "none".parse().unwrap());
            return (headers, Html(String::new())).into_response();
        };

        let kunci = kunci_raw.trim().to_lowercase();

        if kategori == "Pilihan Ganda" {
            if !matches!(kunci.as_str(), "a" | "b" | "c" | "d" | "e") {
                let mut headers = flash_error("Kunci jawaban tidak valid untuk Pilihan Ganda.");
                headers.insert("HX-Retarget", "#soal-form-container".parse().unwrap());
                headers.insert("HX-Reswap", "none".parse().unwrap());
                return (headers, Html(String::new())).into_response();
            }
        } else if kategori == "Pilihan Ganda Kompleks" {
            let parts: Vec<String> = kunci
                .split(',')
                .map(|s| s.trim().to_lowercase())
                .filter(|s| !s.is_empty())
                .collect();
            if parts.len() != 2 {
                let mut headers = flash_error(
                    "Kunci Pilihan Ganda Kompleks harus berisi tepat 2 opsi (mis. a,c).",
                );
                headers.insert("HX-Retarget", "#soal-form-container".parse().unwrap());
                headers.insert("HX-Reswap", "none".parse().unwrap());
                return (headers, Html(String::new())).into_response();
            }
            for p in &parts {
                if !matches!(p.as_str(), "a" | "b" | "c" | "d" | "e") {
                    let mut headers =
                        flash_error("Kunci Pilihan Ganda Kompleks berisi opsi tidak valid.");
                    headers.insert("HX-Retarget", "#soal-form-container".parse().unwrap());
                    headers.insert("HX-Reswap", "none".parse().unwrap());
                    return (headers, Html(String::new())).into_response();
                }
            }
        } else if kategori == "Benar/Salah" {
            if !matches!(kunci.as_str(), "benar" | "salah") {
                let mut headers = flash_error("Kunci Benar/Salah harus 'benar' atau 'salah'.");
                headers.insert("HX-Retarget", "#soal-form-container".parse().unwrap());
                headers.insert("HX-Reswap", "none".parse().unwrap());
                return (headers, Html(String::new())).into_response();
            }
        } else {
            if !matches!(kunci.as_str(), "a" | "b" | "c" | "d" | "e") {
                let mut headers = flash_error("Kunci jawaban tidak valid.");
                headers.insert("HX-Retarget", "#soal-form-container".parse().unwrap());
                headers.insert("HX-Reswap", "none".parse().unwrap());
                return (headers, Html(String::new())).into_response();
            }
        }
    }

    let (kunci_jawaban, opsi_a, opsi_b, opsi_c, opsi_d, opsi_e) = if kategori == "Uraian" {
        (
            None,
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
        )
    } else {
        (
            form.kunci_jawaban.as_deref(),
            form.opsi_a.clone(),
            form.opsi_b.clone(),
            form.opsi_c.clone(),
            form.opsi_d.clone(),
            form.opsi_e.clone(),
        )
    };

    let mut tx = match db.begin().await {
        Ok(tx) => tx,
        Err(_) => {
            let mut headers = flash_error("Gagal memulai transaksi.");
            headers.insert("HX-Retarget", "#soal-form-container".parse().unwrap());
            headers.insert("HX-Reswap", "none".parse().unwrap());
            return (headers, Html(String::new())).into_response();
        }
    };

    eprintln!(
        "DEBUG soal_store: ujian_id={}, kategori={}, kunci={:?}, bobot_nilai={:?}",
        ujian_id, kategori, form.kunci_jawaban, form.bobot_nilai
    );
    let result = sqlx::query(
        r#"
        INSERT INTO soals (pertanyaan, opsi_a, opsi_b, opsi_c, opsi_d, opsi_e, kunci_jawaban, bobot_nilai, kategori, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, NOW(), NOW())
        "#,
    )
    .bind(&form.pertanyaan)
    .bind(opsi_a)
    .bind(opsi_b)
    .bind(opsi_c)
    .bind(opsi_d)
    .bind(opsi_e)
    .bind(kunci_jawaban)
    .bind(form.bobot_nilai.unwrap_or(1.0))
    .bind(kategori)
    .execute(&mut *tx)
    .await;

    let result = match result {
        Ok(result) => {
            eprintln!(
                "DEBUG soal_store: insert soals OK, last_insert_id={}, rows_affected={}",
                result.last_insert_id(),
                result.rows_affected()
            );
            result
        }
        Err(e) => {
            eprintln!(
                "Error creating soal (insert soals) for ujian_id={}: {:?}",
                ujian_id, e
            );
            let _ = tx.rollback().await;
            let mut headers = flash_error("Gagal menambahkan soal!");
            headers.insert("HX-Retarget", "#soal-form-container".parse().unwrap());
            headers.insert("HX-Reswap", "none".parse().unwrap());
            return (headers, Html(String::new())).into_response();
        }
    };

    let soal_id = result.last_insert_id() as i64;

    let max_urutan: Option<i64> =
        sqlx::query_scalar("SELECT MAX(urutan) FROM ujian_soals WHERE ujian_id = ?")
            .bind(ujian_id)
            .fetch_one(&mut *tx)
            .await
            .ok()
            .flatten();

    let urutan = max_urutan.unwrap_or(0) + 1;

    let link_result = sqlx::query(
        "INSERT INTO ujian_soals (ujian_id, soal_id, urutan, created_at) VALUES (?, ?, ?, NOW())",
    )
    .bind(ujian_id)
    .bind(soal_id)
    .bind(urutan as i32)
    .execute(&mut *tx)
    .await;

    match &link_result {
        Ok(r) => eprintln!(
            "DEBUG soal_store: linked soal to ujian ok, rows_affected={}",
            r.rows_affected()
        ),
        Err(e) => eprintln!(
            "Error linking soal to ujian (ujian_id={}, soal_id={}): {:?}",
            ujian_id, soal_id, e
        ),
    }

    if link_result.is_err() {
        let _ = tx.rollback().await;
        let mut headers = flash_error("Gagal menyimpan relasi soal ke ujian.");
        headers.insert("HX-Retarget", "#soal-form-container".parse().unwrap());
        headers.insert("HX-Reswap", "none".parse().unwrap());
        return (headers, Html(String::new())).into_response();
    }

    if let Err(e) = tx.commit().await {
        eprintln!(
            "Error commit soal transaction (ujian_id={}, soal_id={}): {:?}",
            ujian_id, soal_id, e
        );
        let mut headers = flash_error("Gagal menyimpan soal.");
        headers.insert("HX-Retarget", "#soal-form-container".parse().unwrap());
        headers.insert("HX-Reswap", "none".parse().unwrap());
        return (headers, Html(String::new())).into_response();
    } else {
        eprintln!(
            "DEBUG soal_store: transaction committed successfully for ujian_id={}, soal_id={}",
            ujian_id, soal_id
        );
    }

    eprintln!(
        "DEBUG soal_store: returning success flash for ujian_id={}, soal_id={}",
        ujian_id, soal_id
    );
    let mut headers = flash_success("Soal berhasil ditambahkan!");
    // Trigger a client-side event so the form can be cleared without redirect
    // Use After-Settle so existing HX-Trigger (flash) is preserved
    headers.insert("HX-Trigger-After-Settle", "soal:created".parse().unwrap());
    (headers, Html(String::new())).into_response()
}

#[derive(Serialize)]
struct UploadResponse {
    url: String,
}

pub async fn soal_image_upload(
    Htmx(is_htmx): Htmx,
    Path(ujian_id): Path<i64>,
    axum::Extension(s3): axum::Extension<Option<S3State>>,
    mut multipart: Multipart,
) -> axum::response::Response {
    if !is_htmx {
        return StatusCode::BAD_REQUEST.into_response();
    }

    let Some(s3) = s3 else {
        return (StatusCode::SERVICE_UNAVAILABLE, "S3 belum dikonfigurasi.").into_response();
    };

    let mut data: Option<(Vec<u8>, String)> = None;
    let mut context: Option<String> = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name() == Some("context") {
            if let Ok(text) = field.text().await {
                let cleaned = text.trim().to_lowercase();
                if !cleaned.is_empty() {
                    context = Some(cleaned);
                }
            }
            continue;
        }
        if field.name() != Some("image") {
            continue;
        }
        let content_type = field
            .content_type()
            .map(|v| v.to_string())
            .unwrap_or_default();
        let bytes = match field.bytes().await {
            Ok(b) => b,
            Err(_) => {
                return (StatusCode::BAD_REQUEST, "Gagal membaca file.").into_response();
            }
        };
        data = Some((bytes.to_vec(), content_type));
        break;
    }

    let Some((bytes, content_type)) = data else {
        return (StatusCode::BAD_REQUEST, "File image tidak ditemukan.").into_response();
    };

    const MAX_BYTES: usize = 5 * 1024 * 1024;
    if bytes.len() > MAX_BYTES {
        return (StatusCode::PAYLOAD_TOO_LARGE, "Ukuran gambar maksimal 5MB.").into_response();
    }

    let ext = match content_type.as_str() {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/jpg" => "jpg",
        "image/webp" => "webp",
        "image/gif" => "gif",
        _ => {
            return (
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "Format gambar tidak didukung.",
            )
                .into_response();
        }
    };

    let folder = context
        .as_deref()
        .unwrap_or("misc")
        .replace(|c: char| !c.is_ascii_alphanumeric(), "_");
    let key = format!(
        "smpalfa/ujian/{}/{}/{}.{}",
        ujian_id,
        folder,
        Uuid::new_v4(),
        ext
    );
    let body = ByteStream::from(bytes);

    let result = s3
        .client
        .put_object()
        .bucket(&s3.bucket)
        .key(&key)
        .acl(aws_sdk_s3::types::ObjectCannedAcl::PublicRead)
        .content_type(content_type)
        .body(body)
        .send()
        .await;

    if let Err(_) = result {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Gagal upload ke storage.",
        )
            .into_response();
    }

    let url = format!("{}/{}", s3.public_base_url.trim_end_matches('/'), key);
    Json(UploadResponse { url }).into_response()
}

pub async fn soal_delete(
    Htmx(is_htmx): Htmx,
    Path((ujian_id, soal_id)): Path<(i64, i64)>,
    axum::Extension(db): axum::Extension<MySqlPool>,
) -> axum::response::Response {
    if !is_htmx {
        return StatusCode::BAD_REQUEST.into_response();
    }

    let _ = sqlx::query("DELETE FROM ujian_soals WHERE ujian_id = ? AND soal_id = ?")
        .bind(ujian_id)
        .bind(soal_id)
        .execute(&db)
        .await;

    let result = sqlx::query("DELETE FROM soals WHERE id = ?")
        .bind(soal_id)
        .execute(&db)
        .await;

    match result {
        Ok(_) => {
            let headers = flash_success("Soal berhasil dihapus!");
            (headers, Html(String::new())).into_response()
        }
        Err(e) => {
            eprintln!("Error deleting soal: {:?}", e);
            let headers = flash_error("Gagal menghapus soal!");
            (headers, Html(String::new())).into_response()
        }
    }
}
