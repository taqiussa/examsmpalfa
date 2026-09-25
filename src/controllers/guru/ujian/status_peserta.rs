use axum::{
    extract::{Form, Path, Query},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect},
};
use serde::{Deserialize, Serialize};
use sqlx::MySqlPool;
use tera::Context;

use crate::controllers::guru::absensi_kelas::Htmx;
use crate::utils::{page_context::PageContext, render::render, tahun::data_tahun};

use super::{
    StatusPesertaFilter, UjianOption, fetch_ujian_options, flash_error, flash_success,
    lab_kode_atau_default,
};

#[derive(Serialize)]
struct StatusPesertaData {
    tahun: String,
    lab_kode: String,
    sesi: i32,
    gelombang: i32,
    list_ujian: Vec<UjianOption>,
    selected_ujian_id: i64,
    list_peserta: Vec<PesertaStatusRow>,
}

#[derive(Serialize)]
struct StatusPesertaTableData {
    tahun: String,
    lab_kode: String,
    sesi: i32,
    gelombang: i32,
    list_peserta: Vec<PesertaStatusRow>,
    selected_ujian_id: i64,
}

#[derive(Serialize, sqlx::FromRow)]
struct PesertaStatusRow {
    peserta_id: i64,
    pengerjaan_id: Option<i64>,
    nis: String,
    nama: Option<String>,
    kelas_id: Option<i64>,
    kelas: Option<String>,
    tahun: Option<String>,
    lab_kode: Option<String>,
    sesi: Option<i32>,
    gelombang: Option<i32>,
    status: String,
}

#[derive(Deserialize)]
pub struct ToggleStatusForm {
    status: String,
}

pub async fn status_peserta_page(
    ctx: PageContext,
    axum::Extension(db): axum::Extension<MySqlPool>,
) -> Html<String> {
    let tahun = data_tahun();
    let list_ujian = fetch_ujian_options(&db, &tahun).await;
    let selected_ujian_id = list_ujian
        .iter()
        .find(|u| u.is_active == 1)
        .map(|u| u.id)
        .unwrap_or(0);

    let data = StatusPesertaData {
        tahun,
        lab_kode: "01".to_string(),
        sesi: 1,
        gelombang: 1,
        list_ujian,
        selected_ujian_id,
        list_peserta: Vec::new(),
    };

    render(
        &ctx,
        "guru/ujian/status_peserta.html",
        "Status Peserta",
        data,
    )
}

pub async fn status_peserta_table(
    ctx: PageContext,
    Htmx(is_htmx): Htmx,
    Query(filter): Query<StatusPesertaFilter>,
    axum::Extension(db): axum::Extension<MySqlPool>,
) -> axum::response::Response {
    let tahun = filter.tahun.clone().unwrap_or_else(data_tahun);
    let lab_kode = lab_kode_atau_default(filter.lab_kode.as_deref());
    let sesi = filter.sesi.filter(|v| (1..=4).contains(v)).unwrap_or(1);
    let gelombang = filter
        .gelombang
        .filter(|v| (1..=4).contains(v))
        .unwrap_or(1);
    let list_ujian = fetch_ujian_options(&db, &tahun).await;
    let selected_ujian_id = filter
        .ujian_id
        .or_else(|| list_ujian.iter().find(|u| u.is_active == 1).map(|u| u.id))
        .unwrap_or(0);

    if !is_htmx {
        let url = if selected_ujian_id == 0 {
            format!(
                "/status-peserta?tahun={}&lab_kode={}&sesi={}&gelombang={}",
                tahun, lab_kode, sesi, gelombang
            )
        } else {
            format!(
                "/status-peserta?tahun={}&ujian_id={}&lab_kode={}&sesi={}&gelombang={}",
                tahun, selected_ujian_id, lab_kode, sesi, gelombang
            )
        };
        return Redirect::to(&url).into_response();
    }

    let list_peserta = if selected_ujian_id == 0 {
        Vec::new()
    } else {
        sqlx::query_as::<_, PesertaStatusRow>(
            r#"
            SELECT
                p.id as peserta_id,
                CAST(jp.id AS SIGNED) as pengerjaan_id,
                p.nis,
                u.name as nama,
                CAST(p.kelas_id AS SIGNED) as kelas_id,
                k.nama as kelas,
                p.tahun,
                p.lab_kode,
                CAST(p.sesi AS SIGNED) as sesi,
                CAST(p.gelombang AS SIGNED) as gelombang,
                COALESCE(jp.status, 'not_started') as status
            FROM ujian_pesertas p
            LEFT JOIN users u ON u.nis = p.nis
            LEFT JOIN kelas k ON k.id = p.kelas_id
            LEFT JOIN ujian_pengerjaans jp
                ON jp.ujian_id = ?
               AND jp.nis = p.nis
            WHERE p.tahun = ?
              AND p.lab_kode = ?
              AND p.sesi = ?
              AND p.gelombang = ?
            ORDER BY
                p.tahun ASC,
                p.lab_kode ASC,
                p.sesi ASC,
                p.gelombang ASC,
                CASE
                    WHEN COALESCE(jp.status, 'not_started') = 'submitted' THEN 0
                    WHEN COALESCE(jp.status, 'not_started') = 'started' THEN 1
                    ELSE 2
                END,
                k.nama ASC,
                u.name ASC,
                p.nis ASC
            "#,
        )
        .bind(selected_ujian_id)
        .bind(&tahun)
        .bind(&lab_kode)
        .bind(sesi)
        .bind(gelombang)
        .fetch_all(&db)
        .await
        .unwrap_or_else(|e| {
            eprintln!(
                "ERROR status_peserta_table: failed to fetch peserta for ujian_id={} tahun={} -> {:?}",
                selected_ujian_id, tahun, e
            );
            Vec::new()
        })
    };

    let data = StatusPesertaTableData {
        tahun,
        lab_kode,
        sesi,
        gelombang,
        list_peserta,
        selected_ujian_id,
    };

    let mut tera_ctx = Context::new();
    tera_ctx.insert("data", &data);

    let rendered = ctx
        .tera
        .render("guru/ujian/_status_peserta_table.html", &tera_ctx)
        .unwrap();

    Html(rendered).into_response()
}

pub async fn status_peserta_toggle(
    _ctx: PageContext,
    Htmx(is_htmx): Htmx,
    req_headers: HeaderMap,
    Path(peserta_id): Path<i64>,
    axum::Extension(db): axum::Extension<MySqlPool>,
    Form(form): Form<ToggleStatusForm>,
) -> axum::response::Response {
    if !is_htmx {
        return StatusCode::BAD_REQUEST.into_response();
    }

    let status = form.status.trim().to_lowercase();
    if status != "started" && status != "submitted" {
        return (StatusCode::BAD_REQUEST, "Status tidak valid").into_response();
    }

    let result = sqlx::query(
        r#"
        UPDATE ujian_pengerjaans
        SET status = ?,
            submitted_at = CASE WHEN ? = 'submitted' THEN NOW() ELSE NULL END,
            updated_at = NOW()
        WHERE id = ?
        "#,
    )
    .bind(&status)
    .bind(&status)
    .bind(peserta_id)
    .execute(&db)
    .await;

    match result {
        Ok(_) => {
            let mut headers = flash_success("Status peserta diperbarui.");
            let is_status_page = req_headers
                .get("HX-Current-URL")
                .and_then(|v| v.to_str().ok())
                .map(|v| v.contains("/status-peserta"))
                .unwrap_or(false);
            if is_status_page {
                headers.insert(
                    "HX-Trigger-After-Settle",
                    "refresh-status-peserta".parse().unwrap(),
                );
            }
            (headers, Html(String::new())).into_response()
        }
        Err(e) => {
            eprintln!(
                "ERROR status_peserta_toggle: peserta_id={}, err={:?}",
                peserta_id, e
            );
            let mut headers = flash_error("Gagal mengubah status peserta.");
            let is_status_page = req_headers
                .get("HX-Current-URL")
                .and_then(|v| v.to_str().ok())
                .map(|v| v.contains("/status-peserta"))
                .unwrap_or(false);
            if is_status_page {
                headers.insert(
                    "HX-Trigger-After-Settle",
                    "refresh-status-peserta".parse().unwrap(),
                );
            }
            (headers, Html(String::new())).into_response()
        }
    }
}
