use axum::{
    Extension,
    extract::Multipart,
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{Html, IntoResponse, Redirect},
};
use calamine::{Data, Reader, open_workbook_auto_from_rs};
use sqlx::MySqlPool;
use std::{
    collections::{HashMap, HashSet},
    io::{Cursor, Write},
};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

use crate::{
    controllers::admin::tambah_pengguna::Htmx,
    utils::{page_context::PageContext, render::render},
};

#[derive(Debug)]
struct UploadedPesertaRow {
    tahun: String,
    nis: String,
    kelas_id: i64,
    lab_kode: String,
    sesi: i32,
    gelombang: i32,
}

pub async fn download_draft_peserta(
    Extension(db): Extension<MySqlPool>,
) -> axum::response::Response {
    let example = fetch_draft_example(&db).await;
    let Some(example) = example else {
        return (
            StatusCode::NOT_FOUND,
            "Draft tidak dapat dibuat karena data siswa belum tersedia.",
        )
            .into_response();
    };

    let bytes = match build_draft_xlsx(&example) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("ERROR download_draft_peserta: failed generating xlsx: {error:?}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Gagal membuat draft Excel peserta.",
            )
                .into_response();
        }
    };

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        ),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename=\"draft-upload-peserta.xlsx\""),
    );

    (headers, bytes).into_response()
}

async fn fetch_draft_example(db: &MySqlPool) -> Option<UploadedPesertaRow> {
    let registered = sqlx::query_as::<_, (String, String, i64, String, i32, i32)>(
        r#"
        SELECT
            tahun,
            CAST(nis AS CHAR),
            CAST(kelas_id AS SIGNED),
            CAST(lab_kode AS CHAR),
            CAST(sesi AS SIGNED),
            CAST(gelombang AS SIGNED)
        FROM ujian_pesertas
        WHERE kelas_id IS NOT NULL
        ORDER BY updated_at DESC, id DESC
        LIMIT 1
        "#,
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten();

    if let Some((tahun, nis, kelas_id, lab_kode, sesi, gelombang)) = registered {
        return Some(UploadedPesertaRow {
            tahun,
            nis,
            kelas_id,
            lab_kode,
            sesi,
            gelombang,
        });
    }

    sqlx::query_as::<_, (String, String, i64)>(
        r#"
        SELECT CAST(tahun AS CHAR), CAST(nis AS CHAR), CAST(kelas_id AS SIGNED)
        FROM siswas
        ORDER BY tahun DESC, nis ASC
        LIMIT 1
        "#,
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
    .map(|(tahun, nis, kelas_id)| UploadedPesertaRow {
        tahun,
        nis,
        kelas_id,
        lab_kode: "01".to_string(),
        sesi: 1,
        gelombang: 1,
    })
}

fn build_draft_xlsx(example: &UploadedPesertaRow) -> Result<Vec<u8>, zip::result::ZipError> {
    let cursor = Cursor::new(Vec::new());
    let mut archive = ZipWriter::new(cursor);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    write_zip_entry(
        &mut archive,
        "[Content_Types].xml",
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
 <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
 <Default Extension="xml" ContentType="application/xml"/>
 <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
 <Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
</Types>"#,
        options,
    )?;
    write_zip_entry(
        &mut archive,
        "_rels/.rels",
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
 <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>"#,
        options,
    )?;
    write_zip_entry(
        &mut archive,
        "xl/workbook.xml",
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
 <sheets><sheet name="Peserta" sheetId="1" r:id="rId1"/></sheets>
</workbook>"#,
        options,
    )?;
    write_zip_entry(
        &mut archive,
        "xl/_rels/workbook.xml.rels",
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
 <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
</Relationships>"#,
        options,
    )?;

    let worksheet = build_worksheet_xml(example);
    write_zip_entry(
        &mut archive,
        "xl/worksheets/sheet1.xml",
        &worksheet,
        options,
    )?;

    Ok(archive.finish()?.into_inner())
}

fn write_zip_entry(
    archive: &mut ZipWriter<Cursor<Vec<u8>>>,
    name: &str,
    contents: &str,
    options: SimpleFileOptions,
) -> Result<(), zip::result::ZipError> {
    archive.start_file(name, options)?;
    archive.write_all(contents.as_bytes())?;
    Ok(())
}

fn build_worksheet_xml(example: &UploadedPesertaRow) -> String {
    let text_cell = |reference: &str, value: &str| {
        format!(
            r#"<c r="{}" t="inlineStr"><is><t>{}</t></is></c>"#,
            reference,
            escape_xml(value)
        )
    };
    let number_cell =
        |reference: &str, value: i64| format!(r#"<c r="{}"><v>{}</v></c>"#, reference, value);

    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
 <sheetViews><sheetView workbookViewId="0"><pane ySplit="1" topLeftCell="A2" activePane="bottomLeft" state="frozen"/></sheetView></sheetViews>
 <cols><col min="1" max="2" width="18" customWidth="1"/><col min="3" max="6" width="13" customWidth="1"/></cols>
 <sheetData>
  <row r="1">{}{}{}{}{}{}</row>
  <row r="2">{}{}{}{}{}{}</row>
 </sheetData>
 <autoFilter ref="A1:F2"/>
</worksheet>"#,
        text_cell("A1", "tahun"),
        text_cell("B1", "nis"),
        text_cell("C1", "kelas_id"),
        text_cell("D1", "lab_id"),
        text_cell("E1", "gelombang"),
        text_cell("F1", "sesi"),
        text_cell("A2", &example.tahun),
        text_cell("B2", &example.nis),
        number_cell("C2", example.kelas_id),
        text_cell("D2", &example.lab_kode),
        number_cell("E2", i64::from(example.gelombang)),
        number_cell("F2", i64::from(example.sesi)),
    )
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

pub async fn upload_peserta_page(
    ctx: PageContext,
    Htmx(is_htmx): Htmx,
    headers: HeaderMap,
) -> axum::response::Response {
    let is_boosted = headers
        .get("HX-Boosted")
        .and_then(|v| v.to_str().ok())
        .map(|v| v == "true")
        .unwrap_or(false);

    if is_htmx && !is_boosted {
        return Redirect::to("/upload-peserta").into_response();
    }

    let html = render(&ctx, "admin/upload_peserta.html", "Upload Peserta", ());
    Html(html.0).into_response()
}

pub async fn upload_peserta_action(
    _ctx: PageContext,
    Htmx(is_htmx): Htmx,
    Extension(db): Extension<MySqlPool>,
    mut multipart: Multipart,
) -> axum::response::Response {
    let mut file_name = String::new();
    let mut file_bytes = Vec::new();

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "file" => {
                file_name = field.file_name().unwrap_or("").to_string();
                file_bytes = field.bytes().await.unwrap_or_default().to_vec();
            }
            _ => {}
        }
    }

    let lower_name = file_name.to_lowercase();
    if !(lower_name.ends_with(".xlsx") || lower_name.ends_with(".xls")) {
        return upload_error_response(is_htmx, "File harus berformat Excel .xls atau .xlsx.");
    }
    if file_bytes.is_empty() {
        return upload_error_response(is_htmx, "File upload kosong.");
    }

    let rows = match parse_excel_rows(&file_bytes) {
        Ok(rows) => rows,
        Err(message) => {
            return upload_error_response(is_htmx, &message);
        }
    };

    if rows.is_empty() {
        return upload_error_response(is_htmx, "Tidak ada baris peserta yang valid di file Excel.");
    }

    let mut seen = HashSet::new();
    let unique_rows = rows
        .into_iter()
        .filter(|row| {
            seen.insert((
                row.tahun.clone(),
                row.nis.clone(),
                row.lab_kode.clone(),
                row.sesi,
                row.gelombang,
            ))
        })
        .collect::<Vec<_>>();

    let mut tx = match db.begin().await {
        Ok(tx) => tx,
        Err(_) => {
            return upload_error_response(is_htmx, "Gagal memulai transaksi upload peserta.");
        }
    };

    let mut inserted = 0usize;
    for row in unique_rows {
        let exists = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM siswas
            WHERE nis = ? AND tahun = ?
            "#,
        )
        .bind(&row.nis)
        .bind(&row.tahun)
        .fetch_one(&mut *tx)
        .await
        .unwrap_or(0);

        if exists == 0 {
            let _ = tx.rollback().await;
            return upload_error_response(
                is_htmx,
                &format!(
                    "NIS {} pada tahun {} tidak ditemukan di data siswa.",
                    row.nis, row.tahun
                ),
            );
        }

        let res = sqlx::query(
            r#"
            INSERT INTO ujian_pesertas
                (tahun, nis, kelas_id, lab_kode, sesi, gelombang, created_at, updated_at)
            VALUES
                (?, ?, ?, ?, ?, ?, NOW(), NOW())
            ON DUPLICATE KEY UPDATE
                kelas_id = VALUES(kelas_id),
                lab_kode = VALUES(lab_kode),
                sesi = VALUES(sesi),
                gelombang = VALUES(gelombang),
                updated_at = NOW()
            "#,
        )
        .bind(&row.tahun)
        .bind(&row.nis)
        .bind(row.kelas_id)
        .bind(&row.lab_kode)
        .bind(row.sesi)
        .bind(row.gelombang)
        .execute(&mut *tx)
        .await;

        match res {
            Ok(_) => inserted += 1,
            Err(e) => {
                eprintln!(
                    "ERROR upload_peserta_action: failed inserting nis {}: {:?}",
                    row.nis, e
                );
                let _ = tx.rollback().await;
                return upload_error_response(is_htmx, "Gagal menyimpan data peserta ke database.");
            }
        }
    }

    if tx.commit().await.is_err() {
        return upload_error_response(is_htmx, "Gagal menyelesaikan upload peserta.");
    }

    let mut headers = flash_success(&format!(
        "Berhasil memproses {} peserta dari file Excel.",
        inserted
    ));
    if is_htmx {
        headers.insert(
            "HX-Trigger-After-Settle",
            "refresh-progress,refresh-status-peserta".parse().unwrap(),
        );
    }
    (headers, Html(String::new())).into_response()
}
fn upload_error_response(is_htmx: bool, message: &str) -> axum::response::Response {
    let headers = flash_error(message);
    if is_htmx {
        return (headers, Html(String::new())).into_response();
    }
    (headers, Redirect::to("/upload-peserta")).into_response()
}

fn flash_success(message: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        "HX-Trigger",
        format!(
            r#"{{"flash":{{"message":"{}","type":"success"}}}}"#,
            message
        )
        .parse()
        .unwrap(),
    );
    headers
}

fn flash_error(message: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        "HX-Trigger",
        format!(r#"{{"flash":{{"message":"{}","type":"error"}}}}"#, message)
            .parse()
            .unwrap(),
    );
    headers
}

fn parse_excel_rows(bytes: &[u8]) -> Result<Vec<UploadedPesertaRow>, String> {
    let cursor = Cursor::new(bytes.to_vec());
    let mut workbook = open_workbook_auto_from_rs(cursor).map_err(|_| {
        "File Excel tidak bisa dibaca. Pastikan file .xls atau .xlsx valid.".to_string()
    })?;

    let sheet_name = workbook
        .sheet_names()
        .first()
        .cloned()
        .ok_or_else(|| "Workbook Excel tidak memiliki sheet.".to_string())?;

    let range = workbook
        .worksheet_range(&sheet_name)
        .map_err(|_| "Sheet Excel tidak bisa dibaca.".to_string())?;

    let mut rows_iter = range.rows();
    let header_row = rows_iter
        .next()
        .ok_or_else(|| "File Excel tidak memiliki baris header.".to_string())?;

    let mut header_map = HashMap::new();
    for (index, cell) in header_row.iter().enumerate() {
        let key = normalize_header(&cell_to_string(cell));
        if !key.is_empty() {
            header_map.insert(key, index);
        }
    }

    let required_headers = ["tahun", "nis", "kelas_id", "lab_id", "gelombang", "sesi"];
    for header in required_headers {
        if !header_map.contains_key(header) {
            return Err(format!(
                "Header '{}' wajib ada di file Excel. Gunakan heading: tahun, nis, kelas_id, lab_id, gelombang, sesi.",
                header
            ));
        }
    }

    let mut parsed_rows = Vec::new();
    for (row_index, row) in rows_iter.enumerate() {
        if row
            .iter()
            .all(|cell| cell_to_string(cell).trim().is_empty())
        {
            continue;
        }

        let tahun = get_required_string(row, &header_map, "tahun", row_index + 2)?;
        let nis = get_required_string(row, &header_map, "nis", row_index + 2)?;
        let kelas_id = get_required_i64(row, &header_map, "kelas_id", row_index + 2)?;
        let lab_kode = parse_lab_code(
            &get_required_string(row, &header_map, "lab_id", row_index + 2)?,
            row_index + 2,
        )?;
        let gelombang = get_required_i32(row, &header_map, "gelombang", row_index + 2)?;
        let sesi = get_required_i32(row, &header_map, "sesi", row_index + 2)?;

        if !(1..=4).contains(&gelombang) {
            return Err(format!(
                "Gelombang pada baris {} harus 1 sampai 4.",
                row_index + 2
            ));
        }
        if !(1..=4).contains(&sesi) {
            return Err(format!(
                "Sesi pada baris {} harus 1 sampai 4.",
                row_index + 2
            ));
        }

        parsed_rows.push(UploadedPesertaRow {
            tahun,
            nis,
            kelas_id,
            lab_kode,
            sesi,
            gelombang,
        });
    }

    Ok(parsed_rows)
}

fn get_required_string(
    row: &[Data],
    headers: &HashMap<String, usize>,
    key: &str,
    row_number: usize,
) -> Result<String, String> {
    let index = headers
        .get(key)
        .copied()
        .ok_or_else(|| format!("Header '{}' tidak ditemukan.", key))?;
    let value = row.get(index).map(cell_to_string).unwrap_or_default();
    let trimmed = value.trim().to_string();
    if trimmed.is_empty() {
        return Err(format!(
            "Kolom '{}' pada baris {} wajib diisi.",
            key, row_number
        ));
    }
    Ok(trimmed)
}

fn get_required_i64(
    row: &[Data],
    headers: &HashMap<String, usize>,
    key: &str,
    row_number: usize,
) -> Result<i64, String> {
    let raw = get_required_string(row, headers, key, row_number)?;
    raw.parse::<i64>().map_err(|_| {
        format!(
            "Kolom '{}' pada baris {} harus berupa angka.",
            key, row_number
        )
    })
}

fn get_required_i32(
    row: &[Data],
    headers: &HashMap<String, usize>,
    key: &str,
    row_number: usize,
) -> Result<i32, String> {
    let raw = get_required_string(row, headers, key, row_number)?;
    raw.parse::<i32>().map_err(|_| {
        format!(
            "Kolom '{}' pada baris {} harus berupa angka.",
            key, row_number
        )
    })
}

fn normalize_header(value: &str) -> String {
    value
        .trim()
        .to_lowercase()
        .replace(' ', "_")
        .replace('-', "_")
}

fn parse_lab_code(value: &str, row_number: usize) -> Result<String, String> {
    let value = value.trim();
    match value.parse::<u8>() {
        Ok(lab @ 1..=15) => Ok(format!("{lab:02}")),
        _ => Err(format!(
            "Lab pada baris {} harus bernilai 1 sampai 15, ditemukan '{}'.",
            row_number, value
        )),
    }
}

fn cell_to_string(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::String(value) => value.trim().to_string(),
        Data::Float(value) => {
            if value.fract() == 0.0 {
                format!("{:.0}", value)
            } else {
                value.to_string()
            }
        }
        Data::Int(value) => value.to_string(),
        Data::Bool(value) => {
            if *value {
                "1".to_string()
            } else {
                "0".to_string()
            }
        }
        Data::DateTime(value) => value.to_string(),
        Data::DateTimeIso(value) => value.clone(),
        Data::DurationIso(value) => value.clone(),
        Data::Error(_) => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        UploadedPesertaRow, build_draft_xlsx, fetch_draft_example, parse_excel_rows, parse_lab_code,
    };
    use serial_test::serial;

    #[test]
    fn generated_draft_can_be_parsed_by_upload_importer() {
        let example = UploadedPesertaRow {
            tahun: "2026/2027".to_string(),
            nis: "001234".to_string(),
            kelas_id: 7,
            lab_kode: "01".to_string(),
            gelombang: 2,
            sesi: 3,
        };

        let bytes = build_draft_xlsx(&example).expect("draft should be generated");
        let parsed = parse_excel_rows(&bytes).expect("draft should match upload format");

        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].tahun, example.tahun);
        assert_eq!(parsed[0].nis, example.nis);
        assert_eq!(parsed[0].kelas_id, example.kelas_id);
        assert_eq!(parsed[0].lab_kode, example.lab_kode);
        assert_eq!(parsed[0].gelombang, example.gelombang);
        assert_eq!(parsed[0].sesi, example.sesi);
    }

    #[test]
    fn lab_code_accepts_labs_one_through_fifteen() {
        assert_eq!(parse_lab_code("1", 2).unwrap(), "01");
        assert_eq!(parse_lab_code("15", 2).unwrap(), "15");
        assert!(parse_lab_code("16", 2).is_err());
    }

    #[tokio::test]
    #[serial]
    async fn draft_uses_a_student_from_database_when_registry_is_empty() {
        let Some(test_db) = crate::test_support::TestDb::try_new().await else {
            return;
        };
        let seed = crate::test_support::seed_base_data(&test_db.pool).await;

        let example = fetch_draft_example(&test_db.pool)
            .await
            .expect("seeded student should be used as example");

        assert_eq!(example.tahun, seed.tahun);
        assert_eq!(example.nis, seed.nis);
        assert_eq!(example.kelas_id, seed.kelas_id);
        assert_eq!(example.lab_kode, "01");
        assert_eq!(example.gelombang, 1);
        assert_eq!(example.sesi, 1);

        test_db.teardown().await;
    }
}
