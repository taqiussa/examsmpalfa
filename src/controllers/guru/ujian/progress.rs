use axum::{
    extract::Query,
    http::HeaderMap,
    response::{Html, IntoResponse, Redirect},
};
use serde::Serialize;
use sqlx::MySqlPool;

use crate::utils::{page_context::PageContext, render::render, tahun::data_tahun};

use super::{HasilRow, ProgressFilter, TokenRow, UjianDetailRow, UjianOption, fetch_ujian_options};

use crate::controllers::guru::absensi_kelas::Htmx;

#[derive(Serialize)]
struct UjianProgressData {
    tahun: String,
    lab_kode: String,
    sesi: i32,
    gelombang: i32,
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
    // Debug incoming filter
    eprintln!(
        "DEBUG ujian_progress: filter.ujian_id={:?}, filter.tahun={:?}, filter.mata_pelajaran_id={:?}",
        filter.ujian_id, filter.tahun, filter.mata_pelajaran_id
    );

    let tahun = filter.tahun.clone().unwrap_or_else(data_tahun);
    let lab_kode = match filter.lab_kode.as_deref() {
        Some("02") => "02".to_string(),
        _ => "01".to_string(),
    };
    let sesi = filter.sesi.filter(|v| (1..=4).contains(v)).unwrap_or(1);
    let gelombang = filter
        .gelombang
        .filter(|v| (1..=4).contains(v))
        .unwrap_or(1);
    let list_ujian = fetch_ujian_options(&db, &tahun).await;
    // Debug list_ujian contents to ensure mata_pelajaran is present
    eprintln!(
        "DEBUG ujian_progress: list_ujian.count={}",
        list_ujian.len()
    );
    for u in &list_ujian {
        eprintln!(
            "DEBUG ujian_progress: ujian_option id={} mata_pelajaran_id={:?} mata_pelajaran={:?} is_active={}",
            u.id, u.mata_pelajaran_id, u.mata_pelajaran, u.is_active
        );
    }

    let active_ujian = list_ujian.iter().find(|u| u.is_active == 1).cloned();

    let mut selected_ujian: Option<UjianDetailRow> = None;
    let mut list_hasil: Vec<HasilRow> = Vec::new();
    let mut list_token: Vec<TokenRow> = Vec::new();

    // Determine ujian_id to show: prefer explicit ujian_id, otherwise try to pick
    // the first ujian matching provided mata_pelajaran_id (if any)
    let mut ujian_to_show: Option<i64> = filter.ujian_id;
    if ujian_to_show.is_none() {
        if let Some(mapel_id) = filter.mata_pelajaran_id {
            if let Some(found) = list_ujian
                .iter()
                .find(|u| u.mata_pelajaran_id == Some(mapel_id))
            {
                ujian_to_show = Some(found.id);
            }
        }
    }

    eprintln!(
        "DEBUG ujian_progress: resolved ujian_to_show={:?}",
        ujian_to_show
    );

    if let Some(ujian_id) = ujian_to_show {
        let _ = sqlx::query(
            r#"
            UPDATE ujian_pengerjaans p
            LEFT JOIN (
                SELECT
                    j.ujian_id,
                    j.nis,
                    COALESCE(
                        SUM(
                            CAST(
                                COALESCE(j.bobot_nilai, 0) AS DECIMAL(5,2)
                            ),
                            0
                        ),
                        0
                    ) + COALESCE(
                        SUM(
                            CAST(
                                CASE
                                    WHEN j.nilai_uraian IS NOT NULL THEN COALESCE(CAST(j.nilai_uraian AS DECIMAL(5,2)), 0)
                                    ELSE 0
                                END AS DECIMAL(5,2)
                            )
                        ),
                        0
                    ) AS nilai_hitung
                FROM ujian_jawabans j
                JOIN soals s ON s.id = j.soal_id
                WHERE j.ujian_id = ?
                GROUP BY j.ujian_id, j.nis
            ) x ON x.ujian_id = p.ujian_id AND x.nis = p.nis
            SET p.total_nilai = COALESCE(CAST(x.nilai_hitung AS DECIMAL(5,2)), 0),
                p.updated_at = NOW()
            WHERE p.ujian_id = ?
              AND p.status = 'submitted'
            "#,
        )
        .bind(ujian_id)
        .bind(ujian_id)
        .execute(&db)
        .await;

        selected_ujian = match sqlx::query_as::<_, UjianDetailRow>(
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
                mp.nama as mata_pelajaran
            FROM ujians u
            JOIN mata_pelajarans mp ON mp.id = u.mata_pelajaran_id
            WHERE u.id = ?
            "#,
        )
        .bind(ujian_id)
        .fetch_optional(&db)
        .await
        {
            Ok(opt) => opt,
            Err(e) => {
                eprintln!(
                    "ERROR ujian_progress: failed to fetch ujian detail for id={}: {:?}",
                    ujian_id, e
                );
                None
            }
        };

        if selected_ujian.is_some() {
            list_token = sqlx::query_as::<_, TokenRow>(
                r#"
                SELECT
                    id,
                    token,
                    tahun,
                    lab_kode,
                    CAST(sesi AS SIGNED) as sesi,
                    CAST(gelombang AS SIGNED) as gelombang,
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
            .unwrap_or_else(|e| {
                eprintln!(
                    "ERROR ujian_progress: failed to fetch tokens for ujian_id={}: {:?}",
                    ujian_id, e
                );
                Vec::new()
            });

            list_hasil = sqlx::query_as::<_, HasilRow>(
                r#"
                SELECT
                    COALESCE(u.name, CONCAT('NIS ', p.nis)) as nama,
                    p.nis,
                    k.nama as kelas,
                    p.tahun,
                    p.lab_kode,
                    CAST(p.sesi AS SIGNED) as sesi,
                    CAST(p.gelombang AS SIGNED) as gelombang,
                    COALESCE(jp.status, 'not_started') as status,
                    CAST(jp.last_nomor AS SIGNED) as last_nomor,
                    CAST(jp.total_nilai AS DOUBLE) as total_nilai,
                    DATE_FORMAT(jp.submitted_at, '%Y-%m-%d %H:%i') as submitted_at
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
            .bind(ujian_id)
            .bind(&tahun)
            .bind(&lab_kode)
            .bind(sesi)
            .bind(gelombang)
            .fetch_all(&db)
            .await
            .unwrap_or_else(|e| {
                eprintln!(
                    "ERROR ujian_progress: failed to fetch hasil for ujian_id={}: {:?}",
                    ujian_id, e
                );
                Vec::new()
            });
        }
    }

    eprintln!(
        "DEBUG ujian_progress: selected_ujian present={}",
        selected_ujian.is_some()
    );

    let data = UjianProgressData {
        tahun,
        lab_kode,
        sesi,
        gelombang,
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
        let html = render(
            &ctx,
            "guru/ujian/_progress_content.html",
            "Progress Ujian",
            data,
        );
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
