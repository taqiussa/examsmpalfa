use axum::{
    Form,
    extract::Query,
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect},
};
use serde::Serialize;
use sqlx::MySqlPool;
use tera::Context;

use crate::utils::{
    list_kelas::list_kelas, page_context::PageContext, render::render, tahun::data_tahun,
};

use super::{
    MapelOption, UraianFilter, UraianJawabanRow, UraianScoreForm, fetch_mapel_options, flash_error,
};

use crate::controllers::guru::absensi_kelas::Htmx;

#[derive(Serialize)]
struct UraianReviewPageData {
    tahun: String,
    list_kelas: Vec<crate::utils::list_kelas::ListKelas>,
    list_mapel: Vec<MapelOption>,
    kelas_id: i64,
    mata_pelajaran_id: i64,
    require_kelas: bool,
}

pub async fn ujian_uraian_review(
    ctx: PageContext,
    Query(filter): Query<UraianFilter>,
    axum::Extension(db): axum::Extension<MySqlPool>,
) -> Html<String> {
    let tahun = filter.tahun.clone().unwrap_or_else(data_tahun);
    let list_kelas = list_kelas(&db).await.unwrap_or_default();
    let list_mapel = fetch_mapel_options(&db).await;

    let data = UraianReviewPageData {
        tahun,
        list_kelas,
        list_mapel,
        kelas_id: filter.kelas_id.unwrap_or(0),
        mata_pelajaran_id: filter.mata_pelajaran_id.unwrap_or(0),
        require_kelas: false,
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
        let url = format!(
            "/review-uraian?kelas_id={}&mata_pelajaran_id={}&tahun={}",
            filter.kelas_id.unwrap_or(0),
            filter.mata_pelajaran_id.unwrap_or(0),
            filter.tahun.clone().unwrap_or_else(data_tahun)
        );
        return Redirect::to(&url).into_response();
    }

    let kelas_id = filter.kelas_id.unwrap_or(0);
    let mata_pelajaran_id = filter.mata_pelajaran_id.unwrap_or(0);
    let tahun = filter.tahun.clone().unwrap_or_else(data_tahun);
    let mut list: Vec<UraianJawabanRow> = Vec::new();

    if mata_pelajaran_id > 0 {
        let query = sqlx::query_as::<_, UraianJawabanRow>(
            r#"
            SELECT
                j.id as jawaban_id,
                j.ujian_id,
                uj.title as ujian_title,
                k.nama as kelas,
                j.nis,
                u.name as nama,
                j.soal_id,
                s.pertanyaan,
                j.jawaban_uraian,
                COALESCE(CAST(j.nilai_uraian AS SIGNED), 0) as nilai_uraian,
                j.status_uraian,
                COALESCE(CAST(s.bobot_nilai AS DOUBLE), 1) as bobot_nilai
            FROM ujian_jawabans j
            JOIN soals s ON s.id = j.soal_id
            JOIN ujians uj ON uj.id = j.ujian_id
            LEFT JOIN users u ON u.nis = j.nis
            LEFT JOIN siswas sw ON sw.nis = j.nis AND sw.tahun = uj.tahun
            LEFT JOIN kelas k ON k.id = sw.kelas_id
            WHERE s.kategori = 'Uraian'
              AND uj.tahun = ?
              AND uj.mata_pelajaran_id = ?
              AND (? = 0 OR sw.kelas_id = ?)
            ORDER BY
                CASE WHEN j.status_uraian IS NULL THEN 0 ELSE 1 END,
                k.nama ASC,
                uj.title ASC,
                u.name IS NULL,
                u.name ASC,
                j.updated_at DESC
            "#,
        )
        .bind(&tahun)
        .bind(mata_pelajaran_id)
        .bind(kelas_id)
        .bind(kelas_id);

        match query.fetch_all(&db).await {
            Ok(rows) => list = rows,
            Err(e) => {
                eprintln!(
                    "ERROR ujian_uraian_table: query failed tahun={}, kelas_id={}, mata_pelajaran_id={}, err={:?}",
                    tahun, kelas_id, mata_pelajaran_id, e
                );
                list = Vec::new();
            }
        }
    }

    let mut tera_ctx = Context::new();
    tera_ctx.insert("list_jawaban", &list);
    tera_ctx.insert("kelas_id", &kelas_id);
    tera_ctx.insert("mata_pelajaran_id", &mata_pelajaran_id);

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

    eprintln!(
        "DEBUG ujian_uraian_score: START - jawaban_id={}, ujian_id={}, nis={}, nilai={}",
        form.jawaban_id, form.ujian_id, form.nis, form.nilai_uraian
    );

    // Fix: Cast DECIMAL to DOUBLE to avoid type mismatch in sqlx
    let max_bobot: Result<i32, _> = sqlx::query_scalar(
        r#"
        SELECT COALESCE(CAST(s.bobot_nilai AS SIGNED), 1)
        FROM ujian_jawabans j
        JOIN soals s ON s.id = j.soal_id
        WHERE j.id = ?
        LIMIT 1
        "#,
    )
    .bind(form.jawaban_id)
    .fetch_one(&db)
    .await;

    let max_bobot = match max_bobot {
        Ok(val) => val,
        Err(e) => {
            eprintln!("DEBUG ujian_uraian_score: max_bobot query FAILED: {:?}", e);
            1
        }
    };
    eprintln!("DEBUG ujian_uraian_score: max_bobot={}", max_bobot);

    let nilai = form.nilai_uraian.max(0).min(max_bobot);
    eprintln!("DEBUG ujian_uraian_score: final nilai={}", nilai);

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
            eprintln!(
                "DEBUG ujian_uraian_score: UPDATE successful for jawaban_id={}",
                form.jawaban_id
            );

            // Compute decimal totals: total_benar based on bobot_nilai, total_possible_non_uraian, total_uraian
            let summary: Option<(i64, Option<String>)> = sqlx::query_as(
                r#"
                    SELECT
                        CAST(u.mata_pelajaran_id AS SIGNED) as mata_pelajaran_id,
                        u.tahun
                    FROM ujians u
                    WHERE u.id = ?
                    LIMIT 1
                    "#,
            )
            .bind(form.ujian_id)
            .fetch_optional(&db)
            .await
            .unwrap_or(None);

            eprintln!("DEBUG ujian_uraian_score: summary={:?}", summary);

            let mut total_pg_value: f64 = 0.0;
            let mut total_uraian_value: f64 = 0.0;
            if let Some((mata_pelajaran_id, Some(tahun))) = summary {
                eprintln!(
                    "DEBUG ujian_uraian_score: computing totals for mata_pelajaran_id={}, tahun={}",
                    mata_pelajaran_id, tahun
                );

                // Debug: Check what bobot_nilai values exist
                let debug_bobot: Vec<(i64, f64)> = sqlx::query_as(
                    "SELECT j.id, CAST(j.bobot_nilai AS DOUBLE) FROM ujian_jawabans j WHERE j.nis = ? AND j.ujian_id = ?"
                )
                .bind(&form.nis)
                .bind(form.ujian_id)
                .fetch_all(&db)
                .await
                .unwrap_or_default();
                eprintln!(
                    "DEBUG ujian_uraian_score: debug_bobot values = {:?}",
                    debug_bobot
                );

                // Compute total_benar as sum of earned bobot_nilai for this specific ujian and nis
                // (match finalize_submission behavior which sums bobot_nilai per ujian_id)
                let total_benar_result: Result<f64, _> = sqlx::query_scalar(
                    r#"
                    SELECT COALESCE(SUM(CAST(COALESCE(j.bobot_nilai,0) AS DOUBLE)), 0)
                    FROM ujian_jawabans j
                    WHERE j.nis = ?
                      AND j.ujian_id = ?
                    "#,
                )
                .bind(&form.nis)
                .bind(form.ujian_id)
                .fetch_one(&db)
                .await;

                let total_benar = match total_benar_result {
                    Ok(val) => val,
                    Err(e) => {
                        eprintln!(
                            "DEBUG ujian_uraian_score: total_benar query FAILED: {:?}",
                            e
                        );
                        0.0
                    }
                };
                eprintln!("DEBUG ujian_uraian_score: total_benar={}", total_benar);

                // total_possible_non_uraian should be computed for this specific ujian (sum of non-uraian bobot)
                let total_possible_result: Result<f64, _> = sqlx::query_scalar(
                                        r#"
                                        SELECT COALESCE(SUM(CAST(COALESCE(s.bobot_nilai,0) AS DOUBLE)), 0)
                                        FROM ujian_soals us
                                        JOIN soals s ON s.id = us.soal_id
                                        WHERE us.ujian_id = ?
                                            AND s.kategori <> 'Uraian'
                                        "#,
                                )
                                .bind(form.ujian_id)
                                .fetch_one(&db)
                                .await;

                let total_possible_non_uraian = match total_possible_result {
                    Ok(val) => val,
                    Err(e) => {
                        eprintln!(
                            "DEBUG ujian_uraian_score: total_possible query FAILED: {:?}",
                            e
                        );
                        0.0
                    }
                };
                eprintln!(
                    "DEBUG ujian_uraian_score: total_possible_non_uraian={}",
                    total_possible_non_uraian
                );

                let mut total_salah = total_possible_non_uraian - total_benar;
                if total_salah < 0.0 {
                    total_salah = 0.0
                }

                // Sum reviewed uraian scores for this specific ujian and nis
                let total_uraian_result: Result<f64, _> = sqlx::query_scalar(
                                        r#"
                                        SELECT COALESCE(SUM(CAST(COALESCE(j.nilai_uraian,0) AS DOUBLE)), 0)
                                        FROM ujian_jawabans j
                                        WHERE j.nis = ?
                                            AND j.ujian_id = ?
                                            AND j.id IS NOT NULL
                                        "#,
                                )
                                .bind(&form.nis)
                                .bind(form.ujian_id)
                                .fetch_one(&db)
                                .await;

                let total_uraian = match total_uraian_result {
                    Ok(val) => val,
                    Err(e) => {
                        eprintln!(
                            "DEBUG ujian_uraian_score: total_uraian query FAILED: {:?}",
                            e
                        );
                        0.0
                    }
                };
                eprintln!("DEBUG ujian_uraian_score: total_uraian={}", total_uraian);
                eprintln!(
                    "DEBUG ujian_uraian_score: total_nilai will be = total_benar + total_uraian = {} + {} = {}",
                    total_benar,
                    total_uraian,
                    total_benar + total_uraian
                );

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
                .bind(total_benar)
                .bind(total_salah)
                .bind(total_benar)
                .bind(total_uraian)
                .bind(total_benar + total_uraian)
                .execute(&db)
                .await;

                total_pg_value = total_benar;
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
            .bind(total_pg_value + total_uraian_value)
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
