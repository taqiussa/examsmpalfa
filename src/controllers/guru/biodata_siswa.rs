use axum::{
    Extension,
    extract::{FromRequestParts, Query},
    http::{StatusCode, request::Parts},
    response::{Html, IntoResponse, Redirect},
};
use serde::{Deserialize, Serialize};
use sqlx::MySqlPool;
use tera::Context;

use crate::utils::{
    list_kelas::list_kelas,
    page_context::PageContext,
    pagination::{PAGE_SIZE, PaginationConfig, build_pagination},
    render::render,
    tahun::data_tahun,
};

#[derive(Serialize)]
struct BiodataData {
    tahun: String,
    kelas_id: i64,
    list_kelas: Vec<crate::utils::list_kelas::ListKelas>,
}

#[derive(Deserialize)]
pub struct BiodataFilter {
    pub tahun: Option<String>,
    pub kelas_id: Option<String>, // tetap string dari query
    pub page: Option<i64>,
    pub search: Option<String>,
}

#[derive(Serialize)]
struct SiswaRow {
    nis: Option<String>,
    nama: Option<String>,
    kelas: Option<String>,
    tingkat: Option<String>,
    tempat_lahir: Option<String>,
    tanggal_lahir: Option<String>,
    nama_ayah: Option<String>,
    nama_ibu: Option<String>,
    alamat_lengkap: Option<String>,
    telepon: Option<String>,
}

pub struct Htmx(pub bool);

impl<S> FromRequestParts<S> for Htmx
where
    S: Send + Sync,
{
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        let is_htmx = parts
            .headers
            .get("HX-Request")
            .and_then(|v| v.to_str().ok())
            .map(|v| v == "true")
            .unwrap_or(false);

        Ok(Htmx(is_htmx))
    }
}

pub async fn biodata_siswa(ctx: PageContext, Extension(db): Extension<MySqlPool>) -> Html<String> {
    let data = BiodataData {
        tahun: data_tahun(),
        kelas_id: 0,
        list_kelas: list_kelas(&db).await.unwrap_or_default(),
    };

    render(&ctx, "guru/biodata_siswa.html", "Biodata Siswa", data)
}

pub async fn biodata_siswa_table(
    ctx: PageContext,
    Htmx(is_htmx): Htmx,
    Query(filter): Query<BiodataFilter>,
    Extension(db): Extension<MySqlPool>,
) -> axum::response::Response {
    let tahun = filter.tahun.clone().unwrap_or_else(data_tahun);

    // ✅ parse kelas_id dari string → integer
    let kelas_id: i64 = filter
        .kelas_id
        .as_deref()
        .unwrap_or("0")
        .parse()
        .unwrap_or(0);

    let page = filter.page.unwrap_or(1);
    let search = filter.search.clone().unwrap_or_default();

    // redirect jika bukan HTMX
    if !is_htmx {
        let url = format!(
            "/biodata-siswa?tahun={}&kelas_id={}&search={}&page={}",
            tahun, kelas_id, search, page
        );
        return Redirect::to(&url).into_response();
    }

    let search_like = format!("%{}%", search);

    let total: i64 = if kelas_id == 0 {
        sqlx::query_scalar!(
            r#"
            SELECT COUNT(*)
            FROM siswas s
            JOIN users u ON u.nis = s.nis
            WHERE s.tahun = ?
              AND u.name LIKE ?
            "#,
            tahun,
            search_like
        )
        .fetch_one(&db)
        .await
        .unwrap_or(0)
    } else {
        sqlx::query_scalar!(
            r#"
            SELECT COUNT(*)
            FROM siswas s
            JOIN users u ON u.nis = s.nis
            WHERE s.tahun = ?
              AND s.kelas_id = ?
              AND u.name LIKE ?
            "#,
            tahun,
            kelas_id,
            search_like
        )
        .fetch_one(&db)
        .await
        .unwrap_or(0)
    };

    let pagination = build_pagination(page, total);
    let offset = (pagination.page - 1) * PAGE_SIZE;

    let rows: Vec<SiswaRow> = if kelas_id == 0 {
        sqlx::query_as!(
            SiswaRow,
            r#"
            SELECT 
                s.nis,
                u.name AS nama,
                k.nama AS kelas,
                s.tingkat,
                b.tempat_lahir,
                DATE_FORMAT(b.tanggal_lahir, '%Y-%m-%d') AS tanggal_lahir,
                b.nama_ayah,
                b.nama_ibu,
                b.alamat_lengkap,
                b.telepon
            FROM siswas s
            JOIN kelas k ON k.id = s.kelas_id
            JOIN users u ON u.nis = s.nis
            LEFT JOIN biodatas b ON b.nis = s.nis
            WHERE s.tahun = ?
              AND u.name LIKE ?
            ORDER BY k.tingkat, k.nama, u.name
            LIMIT ? OFFSET ?
            "#,
            tahun,
            search_like,
            PAGE_SIZE,
            offset
        )
        .fetch_all(&db)
        .await
        .unwrap_or_default()
    } else {
        sqlx::query_as!(
            SiswaRow,
            r#"
            SELECT 
                s.nis,
                u.name AS nama,
                k.nama AS kelas,
                s.tingkat,
                b.tempat_lahir,
                DATE_FORMAT(b.tanggal_lahir, '%Y-%m-%d') AS tanggal_lahir,
                b.nama_ayah,
                b.nama_ibu,
                b.alamat_lengkap,
                b.telepon
            FROM siswas s
            JOIN kelas k ON k.id = s.kelas_id
            JOIN users u ON u.nis = s.nis
            LEFT JOIN biodatas b ON b.nis = s.nis
            WHERE s.tahun = ?
              AND s.kelas_id = ?
              AND u.name LIKE ?
            ORDER BY k.tingkat, k.nama, u.name
            LIMIT ? OFFSET ?
            "#,
            tahun,
            kelas_id,
            search_like,
            PAGE_SIZE,
            offset
        )
        .fetch_all(&db)
        .await
        .unwrap_or_default()
    };

    let pagination_config = PaginationConfig {
        url: "/biodata-siswa/table".into(),
        target: "#siswa-table".into(),
        extra_query: format!("&tahun={}&kelas_id={}&search={}", tahun, kelas_id, search),
        history_url: format!(
            "/biodata-siswa?tahun={}&kelas_id={}&search={}",
            tahun, kelas_id, search
        ),
    };

    let mut tera_ctx = Context::new();
    tera_ctx.insert("rows", &rows);
    tera_ctx.insert("pagination", &pagination);
    tera_ctx.insert("pagination_config", &pagination_config);

    let rendered = ctx
        .tera
        .render("guru/_biodata_table.html", &tera_ctx)
        .unwrap();

    Html(rendered).into_response()
}
