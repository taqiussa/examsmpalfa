use axum::{
    extract::Query,
    response::{Html, IntoResponse, Redirect},
};
use serde::Serialize;
use sqlx::MySqlPool;
use tera::Context;

use crate::utils::{
    list_kelas::list_kelas, page_context::PageContext, render::render, tahun::data_tahun,
};

use super::{MapelOption, NilaiFilter, NilaiKelasRow, fetch_mapel_options};
use crate::controllers::guru::absensi_kelas::Htmx;

#[derive(Serialize)]
struct NilaiKelasPageData {
    tahun: String,
    list_kelas: Vec<crate::utils::list_kelas::ListKelas>,
    list_mapel: Vec<MapelOption>,
    kelas_id: i64,
    mata_pelajaran_id: i64,
    require_kelas: bool,
}

pub async fn nilai_kelas_mapel_page(
    ctx: PageContext,
    Query(filter): Query<NilaiFilter>,
    axum::Extension(db): axum::Extension<MySqlPool>,
) -> Html<String> {
    let list_kelas = list_kelas(&db).await.unwrap_or_default();
    let list_mapel = fetch_mapel_options(&db).await;
    let tahun = filter.tahun.clone().unwrap_or_else(data_tahun);

    let data = NilaiKelasPageData {
        tahun,
        list_kelas,
        list_mapel,
        kelas_id: filter.kelas_id.unwrap_or(0),
        mata_pelajaran_id: filter.mata_pelajaran_id.unwrap_or(0),
        require_kelas: true,
    };

    render(
        &ctx,
        "guru/ujian/nilai_kelas_mapel.html",
        "Nilai Kelas Mapel",
        data,
    )
}

pub async fn nilai_kelas_mapel_table(
    ctx: PageContext,
    Htmx(is_htmx): Htmx,
    Query(filter): Query<NilaiFilter>,
    axum::Extension(db): axum::Extension<MySqlPool>,
) -> axum::response::Response {
    if !is_htmx {
        let url = format!(
            "/hasil-nilai?kelas_id={}&mata_pelajaran_id={}&tahun={}",
            filter.kelas_id.unwrap_or(0),
            filter.mata_pelajaran_id.unwrap_or(0),
            filter.tahun.clone().unwrap_or_else(data_tahun)
        );
        return Redirect::to(&url).into_response();
    }

    let kelas_id = filter.kelas_id.unwrap_or(0);
    let mata_pelajaran_id = filter.mata_pelajaran_id.unwrap_or(0);
    let tahun = filter.tahun.clone().unwrap_or_else(data_tahun);
    let mut rows: Vec<NilaiKelasRow> = Vec::new();

    if kelas_id > 0 {
        // fetch student list; fill numeric result columns with 0 so mapping to `NilaiKelasRow` succeeds
        rows = sqlx::query_as::<_, NilaiKelasRow>(
            r#"
                        SELECT
                                TRIM(s.nis) as nis,
                                u.name as nama,
                                k.nama as kelas,
                                0.0 as total_benar,
                                0.0 as total_salah,
                                0.0 as nilai_pg,
                                0.0 as nilai_uraian,
                                0.0 as total_nilai
                        FROM siswas s
                        JOIN kelas k ON k.id = s.kelas_id
                        LEFT JOIN users u ON u.nis = s.nis
                        WHERE s.kelas_id = ?
                            AND s.tahun = ?
                        ORDER BY u.name IS NULL, u.name ASC, s.nis ASC
                        "#,
        )
        .bind(kelas_id)
        .bind(&tahun)
        .fetch_all(&db)
        .await
        .unwrap_or_default();

        if mata_pelajaran_id > 0 {
            // fetch hasil_nilais for the selected mata_pelajaran and tahun (one row per nis)
            let nilai_map: std::collections::HashMap<String, (f64, f64, f64, f64, f64)> =
                sqlx::query_as::<_, (String, f64, f64, f64, f64, f64)>(
                    r#"
                    SELECT
                        TRIM(h.nis) as nis,
                        CAST(COALESCE(h.total_benar, 0) AS DOUBLE) as total_benar,
                        CAST(COALESCE(h.total_salah, 0) AS DOUBLE) as total_salah,
                        CAST(COALESCE(h.total_pg, 0) AS DOUBLE) as nilai_pg,
                        CAST(COALESCE(h.total_uraian, 0) AS DOUBLE) as nilai_uraian,
                        CAST(COALESCE(h.total_nilai, 0) AS DOUBLE) as total_nilai
                    FROM hasil_nilais h
                    WHERE h.mata_pelajaran_id = ?
                      AND h.tahun = ?
                    "#,
                )
                .bind(mata_pelajaran_id)
                .bind(&tahun)
                .fetch_all(&db)
                .await
                .unwrap_or_default()
                .into_iter()
                .map(|(nis, benar, salah, pg, uraian, total)| {
                    (nis, (benar, salah, pg, uraian, total))
                })
                .collect();

            for row in &mut rows {
                if let Some((benar, salah, pg, uraian, total)) = nilai_map.get(&row.nis) {
                    row.total_benar = Some(*benar);
                    row.total_salah = Some(*salah);
                    row.nilai_pg = Some(*pg);
                    row.nilai_uraian = Some(*uraian);
                    row.total_nilai = Some(*total);
                }
            }
        }
    }

    let mut tera_ctx = Context::new();
    tera_ctx.insert("rows", &rows);
    tera_ctx.insert("kelas_id", &kelas_id);
    tera_ctx.insert("mata_pelajaran_id", &mata_pelajaran_id);

    let rendered = ctx
        .tera
        .render("guru/ujian/_nilai_kelas_mapel_table.html", &tera_ctx)
        .unwrap();

    Html(rendered).into_response()
}
