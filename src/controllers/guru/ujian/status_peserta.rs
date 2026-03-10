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

use super::{StatusPesertaFilter, UjianOption, fetch_ujian_options, flash_error, flash_success};

#[derive(Serialize)]
struct StatusPesertaData {
    tahun: String,
    list_ujian: Vec<UjianOption>,
    selected_ujian_id: i64,
    list_peserta: Vec<PesertaStatusRow>,
}

#[derive(Serialize)]
struct StatusPesertaTableData {
    list_peserta: Vec<PesertaStatusRow>,
    selected_ujian_id: i64,
}

#[derive(Serialize, sqlx::FromRow)]
struct PesertaStatusRow {
    peserta_id: i64,
    nis: String,
    nama: Option<String>,
    kelas: Option<String>,
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
    let list_ujian = fetch_ujian_options(&db, &tahun).await;
    let selected_ujian_id = filter
        .ujian_id
        .or_else(|| list_ujian.iter().find(|u| u.is_active == 1).map(|u| u.id))
        .unwrap_or(0);

    if !is_htmx {
        let url = if selected_ujian_id == 0 {
            format!("/status-peserta?tahun={}", tahun)
        } else {
            format!(
                "/status-peserta?tahun={}&ujian_id={}",
                tahun, selected_ujian_id
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
                p.nis,
                u.name as nama,
                k.nama as kelas,
                p.status
            FROM ujian_pesertas p
            LEFT JOIN users u ON u.nis = p.nis
            LEFT JOIN siswas s ON s.nis = p.nis AND s.tahun = ?
            LEFT JOIN kelas k ON k.id = s.kelas_id
            WHERE p.ujian_id = ?
            ORDER BY
                CASE WHEN p.status = 'submitted' THEN 0 ELSE 1 END,
                u.name ASC,
                p.nis ASC
            "#,
        )
        .bind(&tahun)
        .bind(selected_ujian_id)
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
        UPDATE ujian_pesertas
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
