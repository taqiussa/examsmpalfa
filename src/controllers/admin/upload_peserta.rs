use axum::{
    Extension,
    extract::Multipart,
    http::HeaderMap,
    response::{Html, IntoResponse, Redirect},
};
use calamine::{Data, Reader, open_workbook_auto_from_rs};
use sqlx::MySqlPool;
use std::{
    collections::{HashMap, HashSet},
    io::Cursor,
};

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
    match value.trim() {
        "1" | "01" => Ok("01".to_string()),
        "2" | "02" => Ok("02".to_string()),
        other => Err(format!(
            "Lab pada baris {} harus bernilai 01/1 atau 02/2, ditemukan '{}'.",
            row_number, other
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
