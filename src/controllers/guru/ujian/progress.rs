use axum::{
    extract::Query,
    http::HeaderMap,
    response::{Html, IntoResponse, Redirect},
};
use serde::Serialize;
use sqlx::MySqlPool;

use crate::utils::{page_context::PageContext, render::render, tahun::data_tahun};

use super::{
    fetch_ujian_options, HasilRow, ProgressFilter, TokenRow, UjianDetailRow, UjianOption,
};

use crate::controllers::guru::absensi_kelas::Htmx;

#[derive(Serialize)]
struct UjianProgressData {
    tahun: String,
    list_ujian: Vec<UjianOption>,
    active_ujian: Option<UjianOption>,
    selected_ujian: Option<UjianDetailRow>,
    list_token: Vec<TokenRow>,
    list_hasil: Vec<HasilRow>,
}

#[derive(Serialize)]
struct UjianActivePanelData {
    active_ujian: Option<UjianOption>,
}

pub async fn ujian_progress(
    ctx: PageContext,
    Htmx(is_htmx): Htmx,
    headers: HeaderMap,
    Query(filter): Query<ProgressFilter>,
    axum::Extension(db): axum::Extension<MySqlPool>,
) -> axum::response::Response {
    let tahun = filter.tahun.clone().unwrap_or_else(data_tahun);
    let list_ujian = fetch_ujian_options(&db, &tahun).await;
    let active_ujian = list_ujian.iter().find(|u| u.is_active == 1).cloned();

    let mut selected_ujian: Option<UjianDetailRow> = None;
    let mut list_hasil: Vec<HasilRow> = Vec::new();
    let mut list_token: Vec<TokenRow> = Vec::new();

    if let Some(ujian_id) = filter.ujian_id {
        let _ = sqlx::query(
            r#"
            UPDATE ujian_pesertas p
            LEFT JOIN (
                SELECT
                    j.ujian_id,
                    j.nis,
                    COALESCE(
                        SUM(
                            CASE
                                WHEN j.pilihan = s.kunci_jawaban THEN COALESCE(s.bobot_nilai, 1)
                                ELSE 0
                            END
                        ),
                        0
                    ) + COALESCE(SUM(COALESCE(j.nilai_uraian, 0)), 0) AS nilai_hitung
                FROM ujian_jawabans j
                JOIN soals s ON s.id = j.soal_id
                WHERE j.ujian_id = ?
                GROUP BY j.ujian_id, j.nis
            ) x ON x.ujian_id = p.ujian_id AND x.nis = p.nis
            SET p.total_nilai = COALESCE(x.nilai_hitung, 0),
                p.updated_at = NOW()
            WHERE p.ujian_id = ?
              AND p.status = 'submitted'
            "#,
        )
        .bind(ujian_id)
        .bind(ujian_id)
        .execute(&db)
        .await;

        selected_ujian = sqlx::query_as::<_, UjianDetailRow>(
            r#"
            SELECT
                u.id,
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

        if selected_ujian.is_some() {
            list_token = sqlx::query_as::<_, TokenRow>(
                r#"
                SELECT
                    id,
                    token,
                    is_active,
                    DATE_FORMAT(expired_at, '%Y-%m-%d %H:%i') as expired_at,
                    DATE_FORMAT(created_at, '%Y-%m-%d %H:%i') as created_at
                FROM ujian_tokens
                WHERE ujian_id = ?
                ORDER BY created_at DESC
                LIMIT 50
                "#,
            )
            .bind(ujian_id)
            .fetch_all(&db)
            .await
            .unwrap_or_default();

            list_hasil = sqlx::query_as::<_, HasilRow>(
                r#"
                SELECT
                    COALESCE(u.name, CONCAT('NIS ', p.nis)) as nama,
                    p.nis,
                    p.status,
                    p.last_nomor,
                    COALESCE(p.total_nilai, 0) as total_nilai,
                    DATE_FORMAT(p.submitted_at, '%Y-%m-%d %H:%i') as submitted_at
                FROM ujian_pesertas p
                LEFT JOIN users u ON u.nis = p.nis
                WHERE p.ujian_id = ?
                ORDER BY
                    CASE WHEN p.status = 'submitted' THEN 0 ELSE 1 END,
                    p.submitted_at DESC,
                    u.name ASC
                "#,
            )
            .bind(ujian_id)
            .fetch_all(&db)
            .await
            .unwrap_or_default();
        }
    }

    let data = UjianProgressData {
        tahun,
        list_ujian,
        active_ujian,
        selected_ujian,
        list_token,
        list_hasil,
    };

    let is_boosted = headers
        .get("HX-Boosted")
        .and_then(|v| v.to_str().ok())
        .map(|v| v == "true")
        .unwrap_or(false);

    if is_htmx && !is_boosted {
        let html = render(&ctx, "guru/ujian/_progress_content.html", "Progress Ujian", data);
        Html(html.0).into_response()
    } else {
        let html = render(&ctx, "guru/ujian/progress.html", "Progress Ujian", data);
        Html(html.0).into_response()
    }
}

pub async fn ujian_progress_active_panel(
    ctx: PageContext,
    Htmx(is_htmx): Htmx,
    Query(filter): Query<ProgressFilter>,
    axum::Extension(db): axum::Extension<MySqlPool>,
) -> axum::response::Response {
    if !is_htmx {
        return Redirect::to("/progress-ujian").into_response();
    }

    let tahun = filter.tahun.clone().unwrap_or_else(data_tahun);
    let list_ujian = fetch_ujian_options(&db, &tahun).await;
    let active_ujian = list_ujian.iter().find(|u| u.is_active == 1).cloned();

    let data = UjianActivePanelData { active_ujian };
    let html = render(
        &ctx,
        "guru/ujian/_active_ujian_panel.html",
        "Progress Ujian",
        data,
    );
    Html(html.0).into_response()
}
