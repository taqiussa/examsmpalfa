use axum::{
    extract::Query,
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect},
    Form,
};
use serde::Serialize;
use sqlx::MySqlPool;
use tera::Context;

use crate::utils::{page_context::PageContext, render::render, tahun::data_tahun};

use super::{
    fetch_ujian_options, flash_error, UraianFilter, UraianJawabanRow, UraianScoreForm, UjianOption,
};

use crate::controllers::guru::absensi_kelas::Htmx;

#[derive(Serialize)]
struct UraianReviewPageData {
    tahun: String,
    list_ujian: Vec<UjianOption>,
    selected_id: i64,
}

pub async fn ujian_uraian_review(
    ctx: PageContext,
    Query(filter): Query<UraianFilter>,
    axum::Extension(db): axum::Extension<MySqlPool>,
) -> Html<String> {
    let tahun = filter.tahun.clone().unwrap_or_else(data_tahun);
    let list_ujian = fetch_ujian_options(&db, &tahun).await;
    let selected_id = filter.ujian_id.unwrap_or(0);

    let data = UraianReviewPageData {
        tahun,
        list_ujian,
        selected_id,
    };

    render(&ctx, "guru/ujian/uraian_review.html", "Review Uraian", data)
}

pub async fn ujian_uraian_table(
    ctx: PageContext,
    Htmx(is_htmx): Htmx,
    Query(filter): Query<UraianFilter>,
    axum::Extension(db): axum::Extension<MySqlPool>,
) -> axum::response::Response {
    if !is_htmx {
        let url = match filter.ujian_id {
            Some(id) => format!(
                "/review-uraian?ujian_id={}&tahun={}",
                id,
                filter.tahun.clone().unwrap_or_else(data_tahun)
            ),
            None => format!(
                "/review-uraian?tahun={}",
                filter.tahun.clone().unwrap_or_else(data_tahun)
            ),
        };
        return Redirect::to(&url).into_response();
    }

    let mut list: Vec<UraianJawabanRow> = Vec::new();
    if let Some(ujian_id) = filter.ujian_id {
        let tahun = filter.tahun.clone().unwrap_or_else(data_tahun);
        list = sqlx::query_as::<_, UraianJawabanRow>(
            r#"
            SELECT
                j.id as jawaban_id,
                j.ujian_id,
                j.nis,
                u.name as nama,
                j.soal_id,
                s.pertanyaan,
                j.jawaban_uraian,
                COALESCE(j.nilai_uraian, 0) as nilai_uraian,
                j.status_uraian,
                COALESCE(s.bobot_nilai, 1) as bobot_nilai
            FROM ujian_jawabans j
            JOIN soals s ON s.id = j.soal_id
            JOIN ujians uj ON uj.id = j.ujian_id
            LEFT JOIN users u ON u.nis = j.nis
            WHERE j.ujian_id = ?
              AND s.kategori = 'Uraian'
              AND uj.tahun = ?
            ORDER BY
                CASE WHEN j.status_uraian IS NULL THEN 0 ELSE 1 END,
                j.updated_at DESC
            "#,
        )
        .bind(ujian_id)
        .bind(&tahun)
        .fetch_all(&db)
        .await
        .unwrap_or_default();
    }

    let mut tera_ctx = Context::new();
    tera_ctx.insert("list_jawaban", &list);
    tera_ctx.insert("selected_id", &filter.ujian_id.unwrap_or(0));

    let rendered = ctx
        .tera
        .render("guru/ujian/_uraian_table.html", &tera_ctx)
        .unwrap();

    Html(rendered).into_response()
}

pub async fn ujian_uraian_score(
    Htmx(is_htmx): Htmx,
    axum::Extension(db): axum::Extension<MySqlPool>,
    Form(form): Form<UraianScoreForm>,
) -> axum::response::Response {
    if !is_htmx {
        return StatusCode::BAD_REQUEST.into_response();
    }

    let max_bobot: i32 = sqlx::query_scalar(
        r#"
        SELECT COALESCE(s.bobot_nilai, 1)
        FROM ujian_jawabans j
        JOIN soals s ON s.id = j.soal_id
        WHERE j.id = ?
        LIMIT 1
        "#,
    )
    .bind(form.jawaban_id)
    .fetch_one(&db)
    .await
    .unwrap_or(1);

    let nilai = form.nilai_uraian.max(0).min(max_bobot);

    let result = sqlx::query(
        r#"
        UPDATE ujian_jawabans
        SET nilai_uraian = ?,
            status_uraian = 'reviewed',
            updated_at = NOW()
        WHERE id = ?
        "#,
    )
    .bind(nilai)
    .bind(form.jawaban_id)
    .execute(&db)
    .await;

    match result {
        Ok(_) => {
            let summary: Option<(i64, Option<String>, i64, i64, i64, i64)> = sqlx::query_as(
                r#"
                SELECT
                    CAST(u.mata_pelajaran_id AS SIGNED) as mata_pelajaran_id,
                    u.tahun,
                    COALESCE(SUM(CASE WHEN s.kategori = 'Pilihan Ganda' AND j.is_benar = 1 THEN 1 ELSE 0 END), 0) as total_benar,
                    COALESCE(SUM(CASE WHEN s.kategori = 'Pilihan Ganda' AND j.is_benar = 0 THEN 1 ELSE 0 END), 0) as total_salah,
                    COALESCE(SUM(CASE WHEN s.kategori = 'Pilihan Ganda' THEN COALESCE(j.bobot_nilai, 0) ELSE 0 END), 0) as total_pg,
                    COALESCE(SUM(CASE WHEN s.kategori = 'Uraian' THEN COALESCE(j.nilai_uraian, 0) ELSE 0 END), 0) as total_uraian
                FROM ujian_jawabans j
                JOIN soals s ON s.id = j.soal_id
                JOIN ujians u ON u.id = j.ujian_id
                WHERE j.ujian_id = ? AND j.nis = ?
                GROUP BY u.mata_pelajaran_id, u.tahun
                LIMIT 1
                "#,
            )
            .bind(form.ujian_id)
            .bind(&form.nis)
            .fetch_optional(&db)
            .await
            .unwrap_or(None);

            let mut total_pg_value: i64 = 0;
            let mut total_uraian_value: i64 = 0;
            if let Some((mata_pelajaran_id, Some(tahun), total_benar, total_salah, total_pg, total_uraian)) = summary {
                let _ = sqlx::query(
                    r#"
                    INSERT INTO hasil_nilais
                        (nis, ujian_id, mata_pelajaran_id, tahun, total_benar, total_salah, total_pg, total_uraian, total_nilai, created_at, updated_at)
                    VALUES
                        (?, ?, ?, ?, ?, ?, ?, ?, ?, NOW(), NOW())
                    ON DUPLICATE KEY UPDATE
                        total_benar = VALUES(total_benar),
                        total_salah = VALUES(total_salah),
                        total_uraian = VALUES(total_uraian),
                        total_pg = VALUES(total_pg),
                        total_nilai = VALUES(total_nilai),
                        updated_at = NOW()
                    "#,
                )
                .bind(&form.nis)
                .bind(form.ujian_id)
                .bind(mata_pelajaran_id)
                .bind(&tahun)
                .bind(total_benar as i32)
                .bind(total_salah as i32)
                .bind(total_pg as i32)
                .bind(total_uraian as i32)
                .bind((total_pg + total_uraian) as i32)
                .execute(&db)
                .await;
                total_pg_value = total_pg;
                total_uraian_value = total_uraian;
            }

            let _ = sqlx::query(
                r#"
                UPDATE ujian_pesertas
                SET total_nilai = ?,
                    updated_at = NOW()
                WHERE ujian_id = ? AND nis = ?
                "#,
            )
            .bind((total_pg_value + total_uraian_value) as i32)
            .bind(form.ujian_id)
            .bind(&form.nis)
            .execute(&db)
            .await;

            let mut headers = HeaderMap::new();
            headers.insert(
                "HX-Trigger-After-Settle",
                "refresh-uraian,refresh-progress".parse().unwrap(),
            );
            (headers, Html(String::new())).into_response()
        }
        Err(e) => {
            eprintln!("Error saving uraian score: {:?}", e);
            let headers = flash_error("Gagal menyimpan nilai uraian.");
            (headers, Html(String::new())).into_response()
        }
    }
}
