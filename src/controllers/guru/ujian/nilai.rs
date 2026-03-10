use axum::{
    extract::Query,
    http::{HeaderMap, HeaderValue, StatusCode},
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

#[derive(sqlx::FromRow)]
struct ExportMetaRow {
    kelas: String,
    mata_pelajaran: String,
}

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
    let rows = fetch_nilai_kelas_rows(&db, kelas_id, mata_pelajaran_id, &tahun).await;

    let mut tera_ctx = Context::new();
    tera_ctx.insert("rows", &rows);
    tera_ctx.insert("kelas_id", &kelas_id);
    tera_ctx.insert("mata_pelajaran_id", &mata_pelajaran_id);
    tera_ctx.insert("tahun", &tahun);

    let rendered = ctx
        .tera
        .render("guru/ujian/_nilai_kelas_mapel_table.html", &tera_ctx)
        .unwrap();

    Html(rendered).into_response()
}

pub async fn nilai_kelas_mapel_export(
    Query(filter): Query<NilaiFilter>,
    axum::Extension(db): axum::Extension<MySqlPool>,
) -> axum::response::Response {
    let kelas_id = filter.kelas_id.unwrap_or(0);
    if kelas_id <= 0 {
        return (StatusCode::BAD_REQUEST, "kelas_id wajib dipilih").into_response();
    }

    let mata_pelajaran_id = filter.mata_pelajaran_id.unwrap_or(0);
    let tahun = filter.tahun.clone().unwrap_or_else(data_tahun);
    let rows = fetch_nilai_kelas_rows(&db, kelas_id, mata_pelajaran_id, &tahun).await;
    let meta = fetch_export_meta(&db, kelas_id, mata_pelajaran_id).await;
    let body = build_excel_xml(&rows, &meta.kelas, &meta.mata_pelajaran, &tahun);
    let filename = format!(
        "nilai kelas {} {}.xls",
        sanitize_filename_part(&meta.kelas),
        sanitize_filename_part(&meta.mata_pelajaran)
    );

    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        HeaderValue::from_static("application/vnd.ms-excel; charset=utf-8"),
    );
    headers.insert(
        axum::http::header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"{}\"", filename)).unwrap(),
    );

    (headers, body).into_response()
}

async fn fetch_nilai_kelas_rows(
    db: &MySqlPool,
    kelas_id: i64,
    mata_pelajaran_id: i64,
    tahun: &str,
) -> Vec<NilaiKelasRow> {
    let mut rows: Vec<NilaiKelasRow> = Vec::new();

    eprintln!(
        "DEBUG fetch_nilai_kelas_rows: incoming filter -> kelas_id={}, mata_pelajaran_id={}, tahun={}",
        kelas_id, mata_pelajaran_id, tahun
    );

    if kelas_id <= 0 {
        return rows;
    }

    let student_query = sqlx::query_as::<_, NilaiKelasRow>(
        r#"
            SELECT
                TRIM(s.nis) as nis,
                u.name as nama,
                k.nama as kelas,
                CAST(0 AS DOUBLE) as total_benar,
                CAST(0 AS DOUBLE) as total_salah,
                CAST(0 AS DOUBLE) as nilai_pg,
                CAST(0 AS DOUBLE) as nilai_uraian,
                CAST(0 AS DOUBLE) as total_nilai
            FROM siswas s
            JOIN kelas k ON k.id = s.kelas_id
            LEFT JOIN users u ON u.nis = s.nis
            WHERE s.kelas_id = ?
              AND s.tahun = ?
            ORDER BY u.name IS NULL, u.name ASC, s.nis ASC
        "#,
    );

    match student_query.bind(kelas_id).bind(tahun).fetch_all(db).await {
        Ok(fetched) => {
            eprintln!(
                "DEBUG fetch_nilai_kelas_rows: fetched {} students for kelas_id={}",
                fetched.len(),
                kelas_id
            );
            rows = fetched;
        }
        Err(e) => {
            eprintln!(
                "ERROR fetch_nilai_kelas_rows: failed to fetch students for kelas_id={} tahun={} -> {:?}",
                kelas_id, tahun, e
            );
            return Vec::new();
        }
    }

    if mata_pelajaran_id <= 0 {
        return rows;
    }

    let hasil_query = sqlx::query_as::<_, (String, f64, f64, f64, f64, f64)>(
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
    );

    match hasil_query
        .bind(mata_pelajaran_id)
        .bind(tahun)
        .fetch_all(db)
        .await
    {
        Ok(fetched_hasil) => {
            eprintln!(
                "DEBUG fetch_nilai_kelas_rows: fetched {} hasil_nilais rows for mata_pelajaran_id={} tahun={}",
                fetched_hasil.len(),
                mata_pelajaran_id,
                tahun
            );

            let nilai_map: std::collections::HashMap<String, (f64, f64, f64, f64, f64)> =
                fetched_hasil
                    .into_iter()
                    .map(|(nis, benar, salah, pg, uraian, total)| {
                        (nis, (benar, salah, pg, uraian, total))
                    })
                    .collect();

            let mut updated = 0usize;
            for row in &mut rows {
                if let Some((benar, salah, pg, uraian, total)) = nilai_map.get(&row.nis) {
                    row.total_benar = Some(*benar);
                    row.total_salah = Some(*salah);
                    row.nilai_pg = Some(*pg);
                    row.nilai_uraian = Some(*uraian);
                    row.total_nilai = Some(*total);
                    updated += 1;
                }
            }

            eprintln!(
                "DEBUG fetch_nilai_kelas_rows: populated nilai for {} of {} students",
                updated,
                rows.len()
            );
        }
        Err(e) => {
            eprintln!(
                "ERROR fetch_nilai_kelas_rows: failed to fetch hasil_nilais for mata_pelajaran_id={} tahun={} -> {:?}",
                mata_pelajaran_id, tahun, e
            );
        }
    }

    rows
}

fn build_excel_xml(
    rows: &[NilaiKelasRow],
    kelas: &str,
    mata_pelajaran: &str,
    tahun: &str,
) -> String {
    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<?mso-application progid="Excel.Sheet"?>
<Workbook xmlns="urn:schemas-microsoft-com:office:spreadsheet"
 xmlns:o="urn:schemas-microsoft-com:office:office"
 xmlns:x="urn:schemas-microsoft-com:office:excel"
 xmlns:ss="urn:schemas-microsoft-com:office:spreadsheet">
 <Worksheet ss:Name="Nilai">
  <Table>
"#,
    );

    push_string_row(&mut xml, &["Rekap Nilai Siswa"]);
    push_string_row(
        &mut xml,
        &[&format!(
            "Tahun: {} | Kelas: {} | Mata Pelajaran: {}",
            tahun, kelas, mata_pelajaran
        )],
    );
    push_string_row(&mut xml, &[]);
    push_string_row(
        &mut xml,
        &[
            "No",
            "NIS",
            "Nama",
            "Kelas",
            "Benar",
            "Salah",
            "Nilai PG",
            "Nilai Uraian",
            "Total Nilai",
        ],
    );

    for (index, row) in rows.iter().enumerate() {
        xml.push_str("   <Row>");
        push_number_cell(&mut xml, (index + 1) as f64);
        push_string_cell(&mut xml, &row.nis);
        let fallback_name = format!("NIS {}", row.nis);
        push_string_cell(&mut xml, row.nama.as_deref().unwrap_or(&fallback_name));
        push_string_cell(&mut xml, row.kelas.as_deref().unwrap_or("-"));
        push_number_or_empty_cell(&mut xml, row.total_benar);
        push_number_or_empty_cell(&mut xml, row.total_salah);
        push_number_or_empty_cell(&mut xml, row.nilai_pg);
        push_number_or_empty_cell(&mut xml, row.nilai_uraian);
        push_number_or_empty_cell(&mut xml, row.total_nilai);
        xml.push_str("</Row>\n");
    }

    xml.push_str("  </Table>\n </Worksheet>\n</Workbook>\n");
    xml
}

fn push_string_row(xml: &mut String, values: &[&str]) {
    xml.push_str("   <Row>");
    if values.is_empty() {
        push_string_cell(xml, "");
    } else {
        for value in values {
            push_string_cell(xml, value);
        }
    }
    xml.push_str("</Row>\n");
}

fn push_string_cell(xml: &mut String, value: &str) {
    xml.push_str(&format!(
        r#"<Cell><Data ss:Type="String">{}</Data></Cell>"#,
        escape_xml(value)
    ));
}

fn push_number_cell(xml: &mut String, value: f64) {
    xml.push_str(&format!(
        r#"<Cell><Data ss:Type="Number">{}</Data></Cell>"#,
        value
    ));
}

fn push_number_or_empty_cell(xml: &mut String, value: Option<f64>) {
    match value {
        Some(number) => push_number_cell(xml, number),
        None => push_string_cell(xml, "-"),
    }
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

async fn fetch_export_meta(db: &MySqlPool, kelas_id: i64, mata_pelajaran_id: i64) -> ExportMetaRow {
    let kelas = sqlx::query_scalar::<_, String>("SELECT nama FROM kelas WHERE id = ? LIMIT 1")
        .bind(kelas_id)
        .fetch_optional(db)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| format!("Kelas {}", kelas_id));

    let mata_pelajaran = if mata_pelajaran_id > 0 {
        sqlx::query_scalar::<_, String>("SELECT nama FROM mata_pelajarans WHERE id = ? LIMIT 1")
            .bind(mata_pelajaran_id)
            .fetch_optional(db)
            .await
            .ok()
            .flatten()
            .unwrap_or_else(|| format!("Mapel {}", mata_pelajaran_id))
    } else {
        "Semua Mata Pelajaran".to_string()
    };

    ExportMetaRow {
        kelas,
        mata_pelajaran,
    }
}

fn sanitize_filename_part(value: &str) -> String {
    let cleaned = value
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => ' ',
            _ => c,
        })
        .collect::<String>();

    cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
}
