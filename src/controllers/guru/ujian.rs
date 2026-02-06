use axum::{
    extract::{Form, Path, Query},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect},
};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use sqlx::MySqlPool;
use tera::Context;

use crate::utils::{page_context::PageContext, render::render, tahun::data_tahun};

use super::absensi_kelas::Htmx;

// ============ STRUCTS ============

#[derive(Serialize)]
struct UjianListData {
    tahun: String,
    list_ujian: Vec<UjianRow>,
}

#[derive(Serialize)]
struct UjianRow {
    id: Option<i64>,
    title: Option<String>,
    description: Option<String>,
    tanggal: Option<NaiveDate>,
    waktu_menit: Option<i32>,
    total_soal: Option<i32>,
    is_active: Option<i8>,
    created_by: Option<i64>,
    created_at: Option<String>,
}

#[derive(Deserialize)]
pub struct UjianFilter {
    pub tahun: Option<String>,
    pub search: Option<String>,
}

#[derive(Serialize)]
struct CreateUjianData {
    tahun: String,
    title: String,
    description: String,
    tanggal: String,
    waktu_menit: i32,
}

#[derive(Deserialize)]
pub struct CreateUjianForm {
    pub title: String,
    pub description: Option<String>,
    pub tanggal: String,
    pub waktu_menit: i32,
}

#[derive(Serialize)]
struct UjianDetailData {
    ujian: Option<UjianDetailRow>,
    list_soal: Vec<SoalRow>,
    can_add_soal: bool,
}

#[derive(Serialize)]
struct UjianDetailRow {
    id: i64,
    title: String,
    description: Option<String>,
    tanggal: NaiveDate,
    waktu_menit: i32,
    total_soal: Option<i32>,
    is_active: Option<i8>,
}

#[derive(Serialize)]
struct SoalRow {
    id: Option<i64>,
    pertanyaan: Option<String>,
    opsi_a: Option<String>,
    opsi_b: Option<String>,
    opsi_c: Option<String>,
    opsi_d: Option<String>,
    kunci_jawaban: Option<String>,
    bobot_nilai: Option<i32>,
    kategori: Option<String>,
    urutan: Option<i32>,
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
    kunci_jawaban: String,
    bobot_nilai: i32,
    kategori: String,
}

#[derive(Deserialize)]
pub struct CreateSoalForm {
    pub pertanyaan: String,
    pub opsi_a: String,
    pub opsi_b: String,
    pub opsi_c: String,
    pub opsi_d: String,
    pub kunci_jawaban: String,
    pub bobot_nilai: Option<i32>,
    pub kategori: Option<String>,
}

// ============ UTILITY FUNCTIONS ============

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

// ============ INDEX & TABLE ============

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

    let rows: Vec<UjianRow> = sqlx::query_as!(
        UjianRow,
        r#"
        SELECT 
            id,
            title,
            description,
            tanggal,
            waktu_menit,
            total_soal,
            is_active,
            created_by,
            DATE_FORMAT(created_at, '%Y-%m-%d %H:%i') as created_at
        FROM ujians
        WHERE (title LIKE ? OR description LIKE ?)
        ORDER BY created_at DESC
        LIMIT 100
        "#,
        search_like,
        search_like
    )
    .fetch_all(&db)
    .await
    .unwrap_or_default();

    let mut tera_ctx = Context::new();
    tera_ctx.insert("list_ujian", &rows);
    tera_ctx.insert("filter_tahun", &tahun);
    tera_ctx.insert("filter_search", &search);

    let rendered = ctx
        .tera
        .render("guru/_ujian_table.html", &tera_ctx)
        .unwrap();

    Html(rendered).into_response()
}

// ============ CREATE & STORE ============

pub async fn ujian_create(ctx: PageContext) -> Html<String> {
    let data = CreateUjianData {
        tahun: data_tahun(),
        title: String::new(),
        description: String::new(),
        tanggal: chrono::Local::now().format("%Y-%m-%d").to_string(),
        waktu_menit: 60,
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

    // Validation
    if form.title.trim().is_empty() {
        let data = CreateUjianData {
            tahun: data_tahun(),
            title: form.title,
            description: form.description.unwrap_or_default(),
            tanggal: form.tanggal,
            waktu_menit: form.waktu_menit,
        };
        let rendered = render(&ctx, "guru/ujian/create.html", "Buat Ujian Baru", data);
        let mut headers = HeaderMap::new();
        headers.insert("HX-Retarget", "#ujian-form".parse().unwrap());
        headers.insert("HX-Reswap", "innerHTML".parse().unwrap());
        return (headers, Html(rendered)).into_response();
    }

    let result = sqlx::query!(
        r#"
        INSERT INTO ujians (title, description, tanggal, waktu_menit, is_active, created_by, created_at, updated_at)
        VALUES (?, ?, ?, ?, TRUE, ?, NOW(), NOW())
        "#,
        form.title,
        form.description,
        form.tanggal,
        form.waktu_menit,
        ctx.user.id as i64
    )
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

// ============ SHOW & DELETE ============

pub async fn ujian_show(
    ctx: PageContext,
    Path(ujian_id): Path<i64>,
    axum::Extension(db): axum::Extension<MySqlPool>,
) -> axum::response::Response {
    let ujian: Option<UjianDetailRow> = sqlx::query_as!(
        UjianDetailRow,
        r#"
        SELECT 
            id,
            title,
            description,
            tanggal,
            waktu_menit,
            total_soal,
            is_active
        FROM ujians
        WHERE id = ?
        "#,
        ujian_id
    )
    .fetch_optional(&db)
    .await
    .unwrap_or(None);

    if ujian.is_none() {
        return Redirect::to("/ujian").into_response();
    }

    let list_soal: Vec<SoalRow> = sqlx::query_as!(
        SoalRow,
        r#"
        SELECT 
            s.id,
            s.pertanyaan,
            s.opsi_a,
            s.opsi_b,
            s.opsi_c,
            s.opsi_d,
            s.kunci_jawaban,
            s.bobot_nilai,
            s.kategori,
            us.urutan
        FROM soals s
        JOIN ujian_soals us ON us.soal_id = s.id
        WHERE us.ujian_id = ?
        ORDER BY us.urutan ASC
        "#,
        ujian_id
    )
    .fetch_all(&db)
    .await
    .unwrap_or_default();

    let data = UjianDetailData {
        ujian,
        list_soal,
        can_add_soal: true,
    };

    let html = render(&ctx, "guru/ujian/show.html", "Detail Ujian", data);
    Html(html.0).into_response()
}

pub async fn ujian_delete(
    Htmx(is_htmx): Htmx,
    Path(ujian_id): Path<i64>,
    axum::Extension(db): axum::Extension<MySqlPool>,
) -> axum::response::Response {
    if !is_htmx {
        return StatusCode::BAD_REQUEST.into_response();
    }

    let result = sqlx::query!("DELETE FROM ujians WHERE id = ?", ujian_id)
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

// ============ SOAL CREATE & STORE ============

pub async fn soal_create(
    ctx: PageContext,
    Htmx(is_htmx): Htmx,
    Path(ujian_id): Path<i64>,
    axum::Extension(db): axum::Extension<MySqlPool>,
) -> axum::response::Response {
    let ujian: Option<UjianDetailRow> = sqlx::query_as!(
        UjianDetailRow,
        r#"
        SELECT 
            id,
            title,
            description,
            tanggal,
            waktu_menit,
            total_soal,
            is_active
        FROM ujians
        WHERE id = ?
        "#,
        ujian_id
    )
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
        kunci_jawaban: "a".to_string(),
        bobot_nilai: 1,
        kategori: String::new(),
    };

    if is_htmx {
        let html = render(&ctx, "guru/ujian/_soal_form.html", "Tambah Soal", data);
        Html(html.0).into_response()
    } else {
        // When requested directly (not via HTMX), render a full page that includes the fragment
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

    // Validation
    if form.pertanyaan.trim().is_empty() {
        let mut headers = flash_error("Pertanyaan tidak boleh kosong!");
        headers.insert("HX-Retarget", "#soal-form-container".parse().unwrap());
        return (headers, Html(String::new())).into_response();
    }

    let result = sqlx::query!(
        r#"
        INSERT INTO soals (pertanyaan, opsi_a, opsi_b, opsi_c, opsi_d, kunci_jawaban, bobot_nilai, kategori, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, NOW(), NOW())
        "#,
        form.pertanyaan,
        form.opsi_a,
        form.opsi_b,
        form.opsi_c,
        form.opsi_d,
        form.kunci_jawaban,
        form.bobot_nilai.unwrap_or(1),
        form.kategori
    )
    .execute(&db)
    .await;

    match result {
        Ok(result) => {
            let soal_id = result.last_insert_id();

            // Get current max urutan
            let max_urutan: Option<i32> = sqlx::query_scalar!(
                "SELECT MAX(urutan) FROM ujian_soals WHERE ujian_id = ?",
                ujian_id
            )
            .fetch_one(&db)
            .await
            .ok()
            .flatten();

            let urutan = max_urutan.unwrap_or(0) + 1;

            // Insert into pivot table
            let _ = sqlx::query!(
                "INSERT INTO ujian_soals (ujian_id, soal_id, urutan, created_at) VALUES (?, ?, ?, NOW())",
                ujian_id,
                soal_id,
                urutan
            )
            .execute(&db)
            .await;

            let mut headers = flash_success("Soal berhasil ditambahkan!");
            headers.insert(
                "HX-Redirect",
                format!("/ujian/{}", ujian_id).parse().unwrap(),
            );
            (headers, Html(String::new())).into_response()
        }
        Err(e) => {
            eprintln!("Error creating soal: {:?}", e);
            let mut headers = flash_error("Gagal menambahkan soal!");
            headers.insert(
                "HX-Redirect",
                format!("/ujian/{}", ujian_id).parse().unwrap(),
            );
            (headers, Html(String::new())).into_response()
        }
    }
}

pub async fn soal_delete(
    Htmx(is_htmx): Htmx,
    Path((ujian_id, soal_id)): Path<(i64, i64)>,
    axum::Extension(db): axum::Extension<MySqlPool>,
) -> axum::response::Response {
    if !is_htmx {
        return StatusCode::BAD_REQUEST.into_response();
    }

    // Delete from pivot table first (due to foreign key)
    let _ = sqlx::query!(
        "DELETE FROM ujian_soals WHERE ujian_id = ? AND soal_id = ?",
        ujian_id,
        soal_id
    )
    .execute(&db)
    .await;

    let result = sqlx::query!("DELETE FROM soals WHERE id = ?", soal_id)
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
