use axum::{
    Extension,
    extract::{FromRequestParts, Query},
    http::{StatusCode, request::Parts},
    response::{Html, IntoResponse, Redirect},
};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, MySqlPool};
use tera::Context;

use crate::utils::{
    page_context::PageContext,
    pagination::{PAGE_SIZE, PaginationConfig, build_pagination},
    render::render,
};

#[derive(Serialize)]
struct DataPeserta {
    tahun: String,
    gelombang: i32,
    lab_kode: String,
    sesi: i32,
    list_tahun: Vec<String>,
}

#[derive(Deserialize)]
pub struct DataPesertaFilter {
    pub tahun: Option<String>,
    pub gelombang: Option<i32>,
    pub lab_kode: Option<String>,
    pub sesi: Option<i32>,
    pub page: Option<i64>,
}

#[derive(Serialize, FromRow)]
struct PesertaRow {
    nis: String,
    nama: Option<String>,
    kelas: Option<String>,
    tahun: String,
    lab_kode: String,
    sesi: i32,
    gelombang: i32,
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

pub async fn data_peserta(
    ctx: PageContext,
    Query(filter): Query<DataPesertaFilter>,
    Extension(db): Extension<MySqlPool>,
) -> Html<String> {
    let list_tahun = load_tahun_options(&db).await;

    let data = DataPeserta {
        tahun: filter.tahun.unwrap_or_default(),
        gelombang: filter.gelombang.unwrap_or(0),
        lab_kode: filter.lab_kode.unwrap_or_default(),
        sesi: filter.sesi.unwrap_or(0),
        list_tahun,
    };

    render(&ctx, "guru/data_peserta.html", "Data Peserta", data)
}

pub async fn data_peserta_table(
    ctx: PageContext,
    Htmx(is_htmx): Htmx,
    Query(filter): Query<DataPesertaFilter>,
    Extension(db): Extension<MySqlPool>,
) -> axum::response::Response {
    let tahun = filter.tahun.unwrap_or_default();
    let gelombang = filter.gelombang.unwrap_or(0);
    let lab_kode = filter.lab_kode.unwrap_or_default();
    let sesi = filter.sesi.unwrap_or(0);
    let page = filter.page.unwrap_or(1);

    if !is_htmx {
        let url = format!(
            "/data-peserta?tahun={}&gelombang={}&lab_kode={}&sesi={}&page={}",
            tahun, gelombang, lab_kode, sesi, page
        );
        return Redirect::to(&url).into_response();
    }

    let total = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM ujian_pesertas p
        LEFT JOIN users u ON u.nis = p.nis
        LEFT JOIN siswas s
            ON s.nis = p.nis
           AND s.tahun = p.tahun
        LEFT JOIN kelas k ON k.id = s.kelas_id
        WHERE (? = '' OR p.tahun = ?)
          AND (? = 0 OR p.gelombang = ?)
          AND (? = '' OR p.lab_kode = ?)
          AND (? = 0 OR p.sesi = ?)
        "#,
    )
    .bind(&tahun)
    .bind(&tahun)
    .bind(gelombang)
    .bind(gelombang)
    .bind(&lab_kode)
    .bind(&lab_kode)
    .bind(sesi)
    .bind(sesi)
    .fetch_one(&db)
    .await
    .unwrap_or(0);

    let pagination = build_pagination(page, total);
    let offset = (pagination.page - 1) * PAGE_SIZE;

    let rows = sqlx::query_as::<_, PesertaRow>(
        r#"
        SELECT
            CAST(p.nis AS CHAR) AS nis,
            CAST(u.name AS CHAR) AS nama,
            CAST(k.nama AS CHAR) AS kelas,
            CAST(p.tahun AS CHAR) AS tahun,
            CAST(p.lab_kode AS CHAR) AS lab_kode,
            CAST(p.sesi AS SIGNED) AS sesi,
            CAST(p.gelombang AS SIGNED) AS gelombang
        FROM ujian_pesertas p
        LEFT JOIN users u ON u.nis = p.nis
        LEFT JOIN siswas s
            ON s.nis = p.nis
           AND s.tahun = p.tahun
        LEFT JOIN kelas k ON k.id = s.kelas_id
        WHERE (? = '' OR p.tahun = ?)
          AND (? = 0 OR p.gelombang = ?)
          AND (? = '' OR p.lab_kode = ?)
          AND (? = 0 OR p.sesi = ?)
        ORDER BY
            p.tahun ASC,
            p.gelombang ASC,
            p.lab_kode ASC,
            p.sesi ASC,
            k.nama ASC,
            u.name ASC,
            p.nis ASC
        LIMIT ? OFFSET ?
        "#,
    )
    .bind(&tahun)
    .bind(&tahun)
    .bind(gelombang)
    .bind(gelombang)
    .bind(&lab_kode)
    .bind(&lab_kode)
    .bind(sesi)
    .bind(sesi)
    .bind(PAGE_SIZE)
    .bind(offset)
    .fetch_all(&db)
    .await
    .unwrap_or_default();

    let pagination_config = PaginationConfig {
        url: "/data-peserta/table".into(),
        target: "#siswa-table".into(),
        extra_query: format!(
            "&tahun={}&gelombang={}&lab_kode={}&sesi={}",
            tahun, gelombang, lab_kode, sesi
        ),
        history_url: format!(
            "/data-peserta?tahun={}&gelombang={}&lab_kode={}&sesi={}",
            tahun, gelombang, lab_kode, sesi
        ),
    };

    let mut tera_ctx = Context::new();
    tera_ctx.insert("rows", &rows);
    tera_ctx.insert("pagination", &pagination);
    tera_ctx.insert("pagination_config", &pagination_config);

    let rendered = ctx
        .tera
        .render("guru/_data_peserta_table.html", &tera_ctx)
        .unwrap();

    Html(rendered).into_response()
}

async fn load_tahun_options(db: &MySqlPool) -> Vec<String> {
    sqlx::query_scalar::<_, String>(
        r#"
        SELECT DISTINCT tahun
        FROM ujian_pesertas
        WHERE tahun IS NOT NULL
          AND tahun <> ''
        ORDER BY tahun DESC
        "#,
    )
    .fetch_all(db)
    .await
    .unwrap_or_default()
}
