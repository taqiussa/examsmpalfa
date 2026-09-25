use axum::{
    Extension, Form,
    extract::Multipart,
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{Html, IntoResponse, Redirect},
};
use bcrypt::{DEFAULT_COST, hash};
use calamine::{Data, Reader, open_workbook_auto_from_rs};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, MySql, MySqlPool, Transaction};
use std::{
    collections::{HashMap, HashSet},
    io::{Cursor, Write},
};
use tera::Context;
use uuid::Uuid;
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

use crate::{
    controllers::guru::absensi_kelas::Htmx,
    utils::{page_context::PageContext, render::render, tahun::data_tahun},
};

const MAX_FILE_BYTES: usize = 5 * 1024 * 1024;
const MAX_IMPORT_ROWS: usize = 2_000;
const MAX_NIS: i64 = 16_777_215;
const DEFAULT_PASSWORD: &str = "12345678";
const USER_MODEL_TYPE: &str = "App\\Models\\User";

#[derive(Debug, Clone, Serialize, FromRow)]
struct KelasDbRow {
    id: i64,
    nama: String,
    tingkat: String,
}

#[derive(Debug, Clone, Serialize)]
struct KelasRef {
    id: i64,
    nama: String,
    tingkat: i32,
}

#[derive(Debug, Clone)]
struct RawStudentRow {
    row_number: usize,
    name: String,
    nis: String,
    kelas_id: String,
    tingkat: String,
    tahun: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ImportStudentRow {
    row_number: usize,
    name: String,
    nis: i64,
    kelas_id: i64,
    tingkat: i32,
    tahun: String,
}

#[derive(Debug, Clone, Serialize)]
struct PreviewStudentRow {
    row_number: usize,
    name: String,
    nis: String,
    kelas_id: String,
    kelas_nama: String,
    tingkat: String,
    tahun: String,
    errors: Vec<String>,
    valid: bool,
}

#[derive(Serialize)]
struct PreviewData {
    general_error: Option<String>,
    rows: Vec<PreviewStudentRow>,
    total_rows: usize,
    valid_rows: usize,
    invalid_rows: usize,
    can_import: bool,
    payload: String,
}

#[derive(Deserialize)]
pub struct ImportForm {
    payload: String,
}

#[derive(Debug, Default, Serialize)]
struct ImportSummary {
    total_rows: usize,
    user_baru: usize,
    user_diperbarui: usize,
    siswa_baru: usize,
    siswa_diperbarui: usize,
    gagal: usize,
}

#[derive(Debug)]
struct ImportFailure {
    row: Option<ImportStudentRow>,
    message: String,
}

#[derive(Serialize)]
struct ImportFailureRow {
    row_number: usize,
    nis: i64,
    name: String,
    error: String,
}

impl ImportFailure {
    fn general(message: impl Into<String>) -> Self {
        Self {
            row: None,
            message: message.into(),
        }
    }

    fn for_row(row: &ImportStudentRow, message: impl Into<String>) -> Self {
        Self {
            row: Some(row.clone()),
            message: message.into(),
        }
    }
}

struct ValidationResult {
    preview: Vec<PreviewStudentRow>,
    valid_rows: Vec<ImportStudentRow>,
}

pub async fn upload_data_siswa_page(ctx: PageContext) -> Html<String> {
    render(
        &ctx,
        "guru/upload_data_siswa/index.html",
        "Upload Data Siswa",
        (),
    )
}

pub async fn download_draft_data_siswa(
    Extension(db): Extension<MySqlPool>,
) -> axum::response::Response {
    let kelas = load_kelas(&db).await;
    if kelas.is_empty() {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            "Draft tidak dapat dibuat karena referensi kelas belum tersedia.",
        )
            .into_response();
    }

    let used_nis =
        sqlx::query_scalar::<_, i64>("SELECT CAST(nis AS SIGNED) FROM users WHERE nis IS NOT NULL")
            .fetch_all(&db)
            .await
            .unwrap_or_default()
            .into_iter()
            .collect::<HashSet<_>>();

    let bytes = match build_draft_xlsx(&kelas, &used_nis, &data_tahun()) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("ERROR download_draft_data_siswa: {error:?}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Gagal membuat draft Excel data siswa.",
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
        HeaderValue::from_static("attachment; filename=\"draft-data-siswa.xlsx\""),
    );
    (headers, bytes).into_response()
}

pub async fn upload_data_siswa_preview(
    ctx: PageContext,
    Htmx(is_htmx): Htmx,
    Extension(db): Extension<MySqlPool>,
    mut multipart: Multipart,
) -> axum::response::Response {
    if !is_htmx {
        return Redirect::to("/upload-data-siswa").into_response();
    }

    let (file_name, file_bytes) = match read_excel_upload(&mut multipart).await {
        Ok(file) => file,
        Err(message) => return render_preview_error(&ctx, &message),
    };

    if !is_excel_file_name(&file_name) {
        return render_preview_error(&ctx, "File harus berformat Excel .xls atau .xlsx.");
    }
    if file_bytes.is_empty() {
        return render_preview_error(&ctx, "File upload kosong.");
    }
    if file_bytes.len() > MAX_FILE_BYTES {
        return render_preview_error(&ctx, "Ukuran file maksimal 5 MB.");
    }

    let raw_rows = match parse_excel_rows(&file_bytes) {
        Ok(rows) => rows,
        Err(message) => return render_preview_error(&ctx, &message),
    };
    if raw_rows.is_empty() {
        return render_preview_error(&ctx, "File Excel tidak memiliki baris data siswa.");
    }
    if raw_rows.len() > MAX_IMPORT_ROWS {
        return render_preview_error(
            &ctx,
            &format!("Maksimal {MAX_IMPORT_ROWS} baris dalam satu kali import."),
        );
    }

    let kelas = load_kelas(&db).await;
    if kelas.is_empty() {
        return render_preview_error(&ctx, "Referensi kelas belum tersedia di database.");
    }

    let validation = validate_rows(raw_rows, &kelas);
    render_preview(&ctx, validation)
}

pub async fn import_data_siswa(
    ctx: PageContext,
    Htmx(is_htmx): Htmx,
    Extension(db): Extension<MySqlPool>,
    Form(form): Form<ImportForm>,
) -> axum::response::Response {
    if !is_htmx {
        return Redirect::to("/upload-data-siswa").into_response();
    }

    let submitted = match serde_json::from_str::<Vec<ImportStudentRow>>(&form.payload) {
        Ok(rows) if !rows.is_empty() && rows.len() <= MAX_IMPORT_ROWS => rows,
        _ => {
            return render_preview_error(
                &ctx,
                "Data preview tidak valid atau kedaluwarsa. Silakan upload ulang file Excel.",
            );
        }
    };

    let kelas = load_kelas(&db).await;
    let raw_rows = submitted
        .into_iter()
        .map(|row| RawStudentRow {
            row_number: row.row_number,
            name: row.name,
            nis: row.nis.to_string(),
            kelas_id: row.kelas_id.to_string(),
            tingkat: row.tingkat.to_string(),
            tahun: row.tahun,
        })
        .collect();
    let validation = validate_rows(raw_rows, &kelas);

    if validation.preview.iter().any(|row| !row.valid) {
        return render_preview(&ctx, validation);
    }

    match execute_import(&db, &validation.valid_rows).await {
        Ok(summary) => render_summary(&ctx, &summary),
        Err(failure) => render_import_failure(&ctx, validation.valid_rows.len(), &failure),
    }
}

async fn read_excel_upload(multipart: &mut Multipart) -> Result<(String, Vec<u8>), String> {
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| "File upload tidak dapat dibaca.".to_string())?
    {
        if field.name() != Some("file") {
            continue;
        }
        let file_name = field.file_name().unwrap_or_default().to_string();
        let bytes = field
            .bytes()
            .await
            .map_err(|_| "File upload tidak dapat dibaca.".to_string())?;
        return Ok((file_name, bytes.to_vec()));
    }

    Err("Pilih file Excel terlebih dahulu.".to_string())
}

fn is_excel_file_name(file_name: &str) -> bool {
    let lower = file_name.trim().to_lowercase();
    lower.ends_with(".xlsx") || lower.ends_with(".xls")
}

fn parse_excel_rows(bytes: &[u8]) -> Result<Vec<RawStudentRow>, String> {
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
    let mut rows = range.rows();
    let header = rows
        .next()
        .ok_or_else(|| "File Excel tidak memiliki baris header.".to_string())?;

    let mut headers = HashMap::new();
    for (index, cell) in header.iter().enumerate() {
        let name = normalize_header(&cell_to_string(cell));
        if !name.is_empty() {
            headers.insert(name, index);
        }
    }
    for required in ["name", "nis", "kelas_id", "tingkat", "tahun"] {
        if !headers.contains_key(required) {
            return Err(format!(
                "Header '{required}' wajib ada. Gunakan heading: name, nis, kelas_id, tingkat, tahun."
            ));
        }
    }

    let mut parsed = Vec::new();
    for (index, row) in rows.enumerate() {
        if row.iter().all(|cell| cell_to_string(cell).is_empty()) {
            continue;
        }
        parsed.push(RawStudentRow {
            row_number: index + 2,
            name: cell_by_header(row, &headers, "name"),
            nis: cell_by_header(row, &headers, "nis"),
            kelas_id: cell_by_header(row, &headers, "kelas_id"),
            tingkat: cell_by_header(row, &headers, "tingkat"),
            tahun: cell_by_header(row, &headers, "tahun"),
        });
    }
    Ok(parsed)
}

fn validate_rows(raw_rows: Vec<RawStudentRow>, kelas: &[KelasRef]) -> ValidationResult {
    let kelas_map = kelas
        .iter()
        .cloned()
        .map(|item| (item.id, item))
        .collect::<HashMap<_, _>>();
    let mut seen = HashSet::new();
    let mut preview = Vec::with_capacity(raw_rows.len());
    let mut valid_rows = Vec::with_capacity(raw_rows.len());

    for raw in raw_rows {
        let name = raw.name.trim().to_string();
        let nis_text = raw.nis.trim().to_string();
        let kelas_text = raw.kelas_id.trim().to_string();
        let tingkat_text = raw.tingkat.trim().to_string();
        let tahun_text = raw.tahun.trim().to_string();
        let mut errors = Vec::new();

        if name.is_empty() {
            errors.push("name wajib diisi".to_string());
        } else if name.chars().count() > 100 {
            errors.push("name maksimal 100 karakter".to_string());
        }

        let nis = parse_nis(&nis_text, &mut errors);
        let kelas_id = parse_positive_i64(&kelas_text, "kelas_id", &mut errors);
        let tingkat = parse_positive_i32(&tingkat_text, "tingkat", &mut errors);
        let tahun = match normalize_tahun(&tahun_text) {
            Ok(value) => Some(value),
            Err(message) => {
                errors.push(message);
                None
            }
        };

        let mut kelas_nama = "-".to_string();
        if let Some(id) = kelas_id {
            match kelas_map.get(&id) {
                Some(kelas_row) => {
                    kelas_nama = kelas_row.nama.clone();
                    if let Some(value) = tingkat
                        && value != kelas_row.tingkat
                    {
                        errors.push(format!(
                            "tingkat {value} tidak sesuai dengan kelas {} (tingkat {})",
                            kelas_row.nama, kelas_row.tingkat
                        ));
                    }
                }
                None => errors.push(format!("kelas_id {id} tidak ditemukan")),
            }
        }

        if let (Some(nis), Some(tahun)) = (nis, tahun.as_ref())
            && !seen.insert((nis, tahun.clone()))
        {
            errors.push("kombinasi NIS dan tahun duplikat di dalam file".to_string());
        }

        let valid = errors.is_empty();
        if valid {
            valid_rows.push(ImportStudentRow {
                row_number: raw.row_number,
                name: name.clone(),
                nis: nis.unwrap(),
                kelas_id: kelas_id.unwrap(),
                tingkat: tingkat.unwrap(),
                tahun: tahun.clone().unwrap(),
            });
        }

        preview.push(PreviewStudentRow {
            row_number: raw.row_number,
            name,
            nis: nis.map(|value| value.to_string()).unwrap_or(nis_text),
            kelas_id: kelas_id
                .map(|value| value.to_string())
                .unwrap_or(kelas_text),
            kelas_nama,
            tingkat: tingkat
                .map(|value| value.to_string())
                .unwrap_or(tingkat_text),
            tahun: tahun.unwrap_or(tahun_text),
            errors,
            valid,
        });
    }

    ValidationResult {
        preview,
        valid_rows,
    }
}

fn parse_nis(value: &str, errors: &mut Vec<String>) -> Option<i64> {
    if value.is_empty() {
        errors.push("nis wajib diisi".to_string());
        return None;
    }
    if !value.chars().all(|character| character.is_ascii_digit()) {
        errors.push("nis harus berupa angka".to_string());
        return None;
    }
    if value.len() > 1 && value.starts_with('0') {
        errors.push(
            "nis tidak boleh diawali 0 karena schema database menyimpan NIS sebagai angka"
                .to_string(),
        );
        return None;
    }
    match value.parse::<i64>() {
        Ok(nis) if (1..=MAX_NIS).contains(&nis) => Some(nis),
        _ => {
            errors.push(format!("nis harus berada pada rentang 1 sampai {MAX_NIS}"));
            None
        }
    }
}

fn parse_positive_i64(value: &str, label: &str, errors: &mut Vec<String>) -> Option<i64> {
    if value.is_empty() {
        errors.push(format!("{label} wajib diisi"));
        return None;
    }
    match value.parse::<i64>() {
        Ok(number) if number > 0 => Some(number),
        _ => {
            errors.push(format!("{label} harus berupa angka positif"));
            None
        }
    }
}

fn parse_positive_i32(value: &str, label: &str, errors: &mut Vec<String>) -> Option<i32> {
    parse_positive_i64(value, label, errors).and_then(|number| match i32::try_from(number) {
        Ok(number) => Some(number),
        Err(_) => {
            errors.push(format!("{label} terlalu besar"));
            None
        }
    })
}

fn normalize_tahun(value: &str) -> Result<String, String> {
    if value.is_empty() {
        return Err("tahun wajib diisi".to_string());
    }
    let parts = value.split('/').map(str::trim).collect::<Vec<_>>();
    if parts.len() != 2 {
        return Err("tahun harus berformat YYYY / YYYY".to_string());
    }
    let start = parts[0]
        .parse::<i32>()
        .map_err(|_| "tahun harus berformat YYYY / YYYY".to_string())?;
    let end = parts[1]
        .parse::<i32>()
        .map_err(|_| "tahun harus berformat YYYY / YYYY".to_string())?;
    if parts[0].len() != 4 || parts[1].len() != 4 || end != start + 1 {
        return Err("tahun harus berformat YYYY / YYYY dan berurutan".to_string());
    }
    Ok(format!("{start} / {end}"))
}

async fn load_kelas(db: &MySqlPool) -> Vec<KelasRef> {
    let rows = sqlx::query_as::<_, KelasDbRow>(
        r#"
        SELECT
            CAST(id AS SIGNED) AS id,
            CAST(nama AS CHAR) AS nama,
            CAST(tingkat AS CHAR) AS tingkat
        FROM kelas
        ORDER BY CAST(tingkat AS UNSIGNED) ASC, nama ASC
        "#,
    )
    .fetch_all(db)
    .await
    .unwrap_or_else(|error| {
        eprintln!("ERROR load_kelas: {error:?}");
        Vec::new()
    });

    rows.into_iter()
        .filter_map(|row| match row.tingkat.trim().parse::<i32>() {
            Ok(tingkat) => Some(KelasRef {
                id: row.id,
                nama: row.nama,
                tingkat,
            }),
            Err(_) => {
                eprintln!(
                    "ERROR load_kelas: tingkat kelas id {} tidak numerik: {}",
                    row.id, row.tingkat
                );
                None
            }
        })
        .collect()
}

async fn execute_import(
    db: &MySqlPool,
    rows: &[ImportStudentRow],
) -> Result<ImportSummary, ImportFailure> {
    let password_hash = hash(DEFAULT_PASSWORD, DEFAULT_COST)
        .map_err(|_| ImportFailure::general("Gagal menyiapkan password akun baru."))?;
    let mut tx = db
        .begin()
        .await
        .map_err(|_| ImportFailure::general("Gagal memulai transaksi import."))?;

    let result = execute_import_transaction(&mut tx, rows, &password_hash).await;
    match result {
        Ok(summary) => {
            tx.commit()
                .await
                .map_err(|_| ImportFailure::general("Gagal menyelesaikan transaksi import."))?;
            Ok(summary)
        }
        Err(message) => {
            let _ = tx.rollback().await;
            Err(message)
        }
    }
}

async fn execute_import_transaction(
    tx: &mut Transaction<'_, MySql>,
    rows: &[ImportStudentRow],
    password_hash: &str,
) -> Result<ImportSummary, ImportFailure> {
    let siswa_role_id = sqlx::query_scalar::<_, i64>(
        "SELECT CAST(id AS SIGNED) FROM roles WHERE name = 'Siswa' LIMIT 1",
    )
    .fetch_optional(&mut **tx)
    .await
    .map_err(|error| {
        eprintln!("ERROR import siswa role query: {error:?}");
        ImportFailure::general("Gagal membaca konfigurasi role Siswa.")
    })?
    .ok_or_else(|| ImportFailure::general("Role Siswa belum tersedia di database."))?;

    let mut existing_users =
        sqlx::query_scalar::<_, i64>("SELECT CAST(nis AS SIGNED) FROM users WHERE nis IS NOT NULL")
            .fetch_all(&mut **tx)
            .await
            .map_err(|error| {
                eprintln!("ERROR import existing users query: {error:?}");
                ImportFailure::general("Gagal membaca data pengguna existing.")
            })?
            .into_iter()
            .collect::<HashSet<_>>();

    let existing_student_rows = sqlx::query_as::<_, (i64, i64, String)>(
        "SELECT CAST(id AS SIGNED), CAST(nis AS SIGNED), CAST(tahun AS CHAR) FROM siswas WHERE nis IS NOT NULL FOR UPDATE",
    )
    .fetch_all(&mut **tx)
    .await
    .map_err(|error| {
        eprintln!("ERROR import existing students query: {error:?}");
        ImportFailure::general("Gagal membaca data siswa existing.")
    })?;
    let mut existing_students = HashMap::new();
    for (id, nis, tahun) in existing_student_rows {
        if existing_students.insert((nis, tahun), id).is_some() {
            return Err(ImportFailure::general(
                "Import dibatalkan karena terdapat duplikasi NIS dan tahun pada database siswa.",
            ));
        }
    }

    let mut summary = ImportSummary {
        total_rows: rows.len(),
        ..ImportSummary::default()
    };

    for row in rows {
        let user_was_existing = existing_users.contains(&row.nis);
        let user_result = sqlx::query(
            r#"
            INSERT INTO users (name, nis, password, created_at, updated_at)
            VALUES (?, ?, ?, NOW(), NOW())
            ON DUPLICATE KEY UPDATE
                id = LAST_INSERT_ID(id),
                name = VALUES(name),
                updated_at = NOW()
            "#,
        )
        .bind(&row.name)
        .bind(row.nis)
        .bind(password_hash)
        .execute(&mut **tx)
        .await
        .map_err(|error| {
            eprintln!(
                "ERROR import user row {} nis {}: {error:?}",
                row.row_number, row.nis
            );
            ImportFailure::for_row(row, "Pengguna gagal disimpan.")
        })?;
        let user_id = user_result.last_insert_id() as i64;

        sqlx::query(
            "INSERT IGNORE INTO model_has_roles (role_id, model_type, model_id) VALUES (?, ?, ?)",
        )
        .bind(siswa_role_id)
        .bind(USER_MODEL_TYPE)
        .bind(user_id)
        .execute(&mut **tx)
        .await
        .map_err(|error| {
            eprintln!(
                "ERROR import role row {} nis {}: {error:?}",
                row.row_number, row.nis
            );
            ImportFailure::for_row(row, "Role Siswa gagal disimpan.")
        })?;

        if user_was_existing {
            summary.user_diperbarui += 1;
        } else {
            summary.user_baru += 1;
            existing_users.insert(row.nis);
        }

        let student_key = (row.nis, row.tahun.clone());
        if let Some(student_id) = existing_students.get(&student_key).copied() {
            sqlx::query(
                r#"
                UPDATE siswas
                SET kelas_id = ?, tingkat = ?, tahun = ?, updated_at = NOW()
                WHERE id = ?
                "#,
            )
            .bind(row.kelas_id)
            .bind(row.tingkat)
            .bind(&row.tahun)
            .bind(student_id)
            .execute(&mut **tx)
            .await
            .map_err(|error| {
                eprintln!(
                    "ERROR import student update row {} nis {}: {error:?}",
                    row.row_number, row.nis
                );
                ImportFailure::for_row(row, "Data siswa gagal diperbarui.")
            })?;
            summary.siswa_diperbarui += 1;
        } else {
            let result = sqlx::query(
                r#"
                INSERT INTO siswas (nis, kelas_id, tingkat, tahun, created_at, updated_at)
                VALUES (?, ?, ?, ?, NOW(), NOW())
                "#,
            )
            .bind(row.nis)
            .bind(row.kelas_id)
            .bind(row.tingkat)
            .bind(&row.tahun)
            .execute(&mut **tx)
            .await
            .map_err(|error| {
                eprintln!(
                    "ERROR import student insert row {} nis {}: {error:?}",
                    row.row_number, row.nis
                );
                ImportFailure::for_row(row, "Data siswa gagal disimpan.")
            })?;
            existing_students.insert(student_key, result.last_insert_id() as i64);
            summary.siswa_baru += 1;
        }
    }

    Ok(summary)
}

fn render_preview(ctx: &PageContext, validation: ValidationResult) -> axum::response::Response {
    let total_rows = validation.preview.len();
    let invalid_rows = validation.preview.iter().filter(|row| !row.valid).count();
    let valid_rows = total_rows - invalid_rows;
    let can_import = total_rows > 0 && invalid_rows == 0;
    let payload = if can_import {
        serde_json::to_string(&validation.valid_rows).unwrap_or_default()
    } else {
        String::new()
    };
    let data = PreviewData {
        general_error: None,
        rows: validation.preview,
        total_rows,
        valid_rows,
        invalid_rows,
        can_import,
        payload,
    };
    Html(render_preview_partial(ctx, &data)).into_response()
}

fn render_preview_error(ctx: &PageContext, message: &str) -> axum::response::Response {
    let data = PreviewData {
        general_error: Some(message.to_string()),
        rows: Vec::new(),
        total_rows: 0,
        valid_rows: 0,
        invalid_rows: 0,
        can_import: false,
        payload: String::new(),
    };
    Html(render_preview_partial(ctx, &data)).into_response()
}

fn render_preview_partial(ctx: &PageContext, data: &PreviewData) -> String {
    let mut tera_ctx = Context::new();
    tera_ctx.insert("data", data);
    ctx.tera
        .render("guru/upload_data_siswa/_preview.html", &tera_ctx)
        .unwrap()
}

fn render_summary(ctx: &PageContext, summary: &ImportSummary) -> axum::response::Response {
    let mut tera_ctx = Context::new();
    tera_ctx.insert("summary", summary);
    Html(
        ctx.tera
            .render("guru/upload_data_siswa/_summary.html", &tera_ctx)
            .unwrap(),
    )
    .into_response()
}

fn render_import_failure(
    ctx: &PageContext,
    total_rows: usize,
    failure: &ImportFailure,
) -> axum::response::Response {
    let summary = ImportSummary {
        total_rows,
        gagal: total_rows,
        ..ImportSummary::default()
    };
    let mut tera_ctx = Context::new();
    tera_ctx.insert("summary", &summary);
    tera_ctx.insert("error", &failure.message);
    if let Some(row) = &failure.row {
        tera_ctx.insert(
            "error_row",
            &ImportFailureRow {
                row_number: row.row_number,
                nis: row.nis,
                name: row.name.clone(),
                error: failure.message.clone(),
            },
        );
    }
    Html(
        ctx.tera
            .render("guru/upload_data_siswa/_summary.html", &tera_ctx)
            .unwrap(),
    )
    .into_response()
}

fn build_draft_xlsx(
    kelas: &[KelasRef],
    used_nis: &HashSet<i64>,
    tahun: &str,
) -> Result<Vec<u8>, zip::result::ZipError> {
    let mut first_nis = 9_000_000 + (Uuid::new_v4().as_u128() % 6_000_000) as i64;
    while used_nis.contains(&first_nis) {
        first_nis = if first_nis >= 14_999_999 {
            9_000_000
        } else {
            first_nis + 1
        };
    }
    let mut second_nis = first_nis + 1;
    while used_nis.contains(&second_nis) || second_nis == first_nis {
        second_nis = if second_nis >= 14_999_999 {
            9_000_000
        } else {
            second_nis + 1
        };
    }
    let examples = [
        ("Contoh Siswa A", first_nis, &kelas[0]),
        (
            "Contoh Siswa B",
            second_nis,
            kelas.get(1).unwrap_or(&kelas[0]),
        ),
    ];

    let mut data_sheet = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
 <sheetViews><sheetView workbookViewId="0"><pane ySplit="1" topLeftCell="A2" activePane="bottomLeft" state="frozen"/></sheetView></sheetViews>
 <cols><col min="1" max="1" width="24" customWidth="1"/><col min="2" max="5" width="18" customWidth="1"/></cols>
 <sheetData>
"#,
    );
    data_sheet.push_str(&xlsx_row(
        1,
        &[
            XlsxValue::Text("name"),
            XlsxValue::Text("nis"),
            XlsxValue::Text("kelas_id"),
            XlsxValue::Text("tingkat"),
            XlsxValue::Text("tahun"),
        ],
    ));
    for (index, (name, nis, kelas)) in examples.iter().enumerate() {
        data_sheet.push_str(&xlsx_row(
            index + 2,
            &[
                XlsxValue::Text(name),
                XlsxValue::TextOwned(nis.to_string()),
                XlsxValue::Number(kelas.id),
                XlsxValue::Number(i64::from(kelas.tingkat)),
                XlsxValue::Text(tahun),
            ],
        ));
    }
    data_sheet.push_str(" </sheetData>\n <autoFilter ref=\"A1:E3\"/>\n</worksheet>");

    let mut reference_sheet = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
 <sheetViews><sheetView workbookViewId="0"><pane ySplit="1" topLeftCell="A2" activePane="bottomLeft" state="frozen"/></sheetView></sheetViews>
 <cols><col min="1" max="1" width="14" customWidth="1"/><col min="2" max="2" width="24" customWidth="1"/><col min="3" max="3" width="14" customWidth="1"/></cols>
 <sheetData>
"#,
    );
    reference_sheet.push_str(&xlsx_row(
        1,
        &[
            XlsxValue::Text("kelas_id"),
            XlsxValue::Text("nama_kelas"),
            XlsxValue::Text("tingkat"),
        ],
    ));
    for (index, item) in kelas.iter().enumerate() {
        reference_sheet.push_str(&xlsx_row(
            index + 2,
            &[
                XlsxValue::Number(item.id),
                XlsxValue::Text(&item.nama),
                XlsxValue::Number(i64::from(item.tingkat)),
            ],
        ));
    }
    reference_sheet.push_str(&format!(
        " </sheetData>\n <autoFilter ref=\"A1:C{}\"/>\n</worksheet>",
        kelas.len() + 1
    ));

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
 <Override PartName="/xl/worksheets/sheet2.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
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
 <sheets>
  <sheet name="Data Siswa" sheetId="1" r:id="rId1"/>
  <sheet name="Referensi Kelas" sheetId="2" r:id="rId2"/>
 </sheets>
</workbook>"#,
        options,
    )?;
    write_zip_entry(
        &mut archive,
        "xl/_rels/workbook.xml.rels",
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
 <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
 <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet2.xml"/>
</Relationships>"#,
        options,
    )?;
    write_zip_entry(
        &mut archive,
        "xl/worksheets/sheet1.xml",
        &data_sheet,
        options,
    )?;
    write_zip_entry(
        &mut archive,
        "xl/worksheets/sheet2.xml",
        &reference_sheet,
        options,
    )?;
    Ok(archive.finish()?.into_inner())
}

enum XlsxValue<'a> {
    Text(&'a str),
    TextOwned(String),
    Number(i64),
}

fn xlsx_row(row_number: usize, values: &[XlsxValue<'_>]) -> String {
    let mut row = format!("  <row r=\"{row_number}\">");
    for (index, value) in values.iter().enumerate() {
        let column = char::from(b'A' + index as u8);
        let reference = format!("{column}{row_number}");
        match value {
            XlsxValue::Text(value) => row.push_str(&xlsx_text_cell(&reference, value)),
            XlsxValue::TextOwned(value) => row.push_str(&xlsx_text_cell(&reference, value)),
            XlsxValue::Number(value) => {
                row.push_str(&format!("<c r=\"{}\"><v>{}</v></c>", reference, value))
            }
        }
    }
    row.push_str("</row>\n");
    row
}

fn xlsx_text_cell(reference: &str, value: &str) -> String {
    format!(
        "<c r=\"{}\" t=\"inlineStr\"><is><t>{}</t></is></c>",
        reference,
        escape_xml(value)
    )
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

fn normalize_header(value: &str) -> String {
    value.trim().to_lowercase().replace([' ', '-'], "_")
}

fn cell_by_header(row: &[Data], headers: &HashMap<String, usize>, key: &str) -> String {
    headers
        .get(key)
        .and_then(|index| row.get(*index))
        .map(cell_to_string)
        .unwrap_or_default()
}

fn cell_to_string(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::String(value) => value.trim().to_string(),
        Data::Float(value) if value.fract() == 0.0 => format!("{value:.0}"),
        Data::Float(value) => value.to_string(),
        Data::Int(value) => value.to_string(),
        Data::Bool(value) => value.to_string(),
        Data::DateTime(value) => value.to_string(),
        Data::DateTimeIso(value) | Data::DurationIso(value) => value.clone(),
        Data::Error(_) => String::new(),
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

#[cfg(test)]
mod tests {
    use super::{
        ImportStudentRow, ImportSummary, KelasRef, RawStudentRow, build_draft_xlsx, execute_import,
        parse_excel_rows, validate_rows,
    };
    use crate::{
        models::{auth_user::AuthUser, role::Role},
        routes::guru_routes::guru_only_routes,
    };
    use axum::{
        Extension, Router,
        body::Body,
        http::{Request, StatusCode},
    };
    use bcrypt::verify;
    use calamine::{Data, Reader, open_workbook_auto_from_rs};
    use serial_test::serial;
    use std::{
        collections::HashSet,
        io::{Cursor, Read, Write},
    };
    use tower::ServiceExt;
    use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

    fn kelas_fixture() -> Vec<KelasRef> {
        vec![
            KelasRef {
                id: 12,
                nama: "8.A".to_string(),
                tingkat: 8,
            },
            KelasRef {
                id: 13,
                nama: "8.B".to_string(),
                tingkat: 8,
            },
        ]
    }

    fn raw_row(row_number: usize, nis: &str, kelas_id: &str, tingkat: &str) -> RawStudentRow {
        RawStudentRow {
            row_number,
            name: "Siswa Contoh".to_string(),
            nis: nis.to_string(),
            kelas_id: kelas_id.to_string(),
            tingkat: tingkat.to_string(),
            tahun: "2026 / 2027".to_string(),
        }
    }

    #[test]
    fn validation_rejects_invalid_nis_unknown_class_and_level_mismatch() {
        let rows = vec![
            raw_row(2, "ABC", "12", "8"),
            raw_row(3, "260002", "999", "8"),
            raw_row(4, "260003", "12", "7"),
            raw_row(5, "0260004", "12", "8"),
        ];
        let validation = validate_rows(rows, &kelas_fixture());

        assert!(
            validation.preview[0]
                .errors
                .iter()
                .any(|e| e.contains("nis harus"))
        );
        assert!(
            validation.preview[1]
                .errors
                .iter()
                .any(|e| e.contains("kelas_id 999 tidak ditemukan"))
        );
        assert!(
            validation.preview[2]
                .errors
                .iter()
                .any(|e| e.contains("tidak sesuai"))
        );
        assert!(
            validation.preview[3]
                .errors
                .iter()
                .any(|e| e.contains("diawali 0"))
        );
        assert!(validation.valid_rows.is_empty());
    }

    #[test]
    fn draft_contains_real_class_reference_and_only_dummy_identity() {
        let classes = kelas_fixture();
        let actual_nis = HashSet::from([9_000_000_i64, 9_000_001_i64, 9_000_002_i64]);
        let bytes = build_draft_xlsx(&classes, &actual_nis, "2026 / 2027").unwrap();
        let mut workbook = open_workbook_auto_from_rs(Cursor::new(bytes.clone())).unwrap();

        assert_eq!(workbook.sheet_names(), &["Data Siswa", "Referensi Kelas"]);
        let data = workbook.worksheet_range("Data Siswa").unwrap();
        assert_eq!(data.get((0, 0)), Some(&Data::String("name".to_string())));
        assert_eq!(
            data.get((1, 0)),
            Some(&Data::String("Contoh Siswa A".to_string()))
        );
        let dummy_nis = super::cell_to_string(data.get((1, 1)).unwrap())
            .parse::<i64>()
            .unwrap();
        assert!(!actual_nis.contains(&dummy_nis));

        let reference = workbook.worksheet_range("Referensi Kelas").unwrap();
        assert_eq!(super::cell_to_string(reference.get((1, 0)).unwrap()), "12");
        assert_eq!(super::cell_to_string(reference.get((1, 1)).unwrap()), "8.A");
        assert_eq!(super::cell_to_string(reference.get((1, 2)).unwrap()), "8");

        let invalid_header = replace_zip_entry(
            &bytes,
            "xl/worksheets/sheet1.xml",
            "<t>name</t>",
            "<t>nama</t>",
        );
        let error = parse_excel_rows(&invalid_header).unwrap_err();
        assert!(error.contains("Header 'name' wajib ada"));
        assert!(parse_excel_rows(b"bukan file excel").is_err());

        let mut summary_context = tera::Context::new();
        summary_context.insert("summary", &ImportSummary::default());
        let summary_html = crate::test_support::build_test_tera()
            .render("guru/upload_data_siswa/_summary.html", &summary_context)
            .unwrap();
        assert!(summary_html.contains("Import data siswa berhasil"));
    }

    fn replace_zip_entry(bytes: &[u8], target: &str, from: &str, to: &str) -> Vec<u8> {
        let mut source = ZipArchive::new(Cursor::new(bytes)).unwrap();
        let mut files = Vec::new();
        for index in 0..source.len() {
            let mut file = source.by_index(index).unwrap();
            let name = file.name().to_string();
            let mut contents = Vec::new();
            file.read_to_end(&mut contents).unwrap();
            if name == target {
                contents = String::from_utf8(contents)
                    .unwrap()
                    .replace(from, to)
                    .into_bytes();
            }
            files.push((name, contents));
        }

        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        for (name, contents) in files {
            writer
                .start_file(name, SimpleFileOptions::default())
                .unwrap();
            writer.write_all(&contents).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    #[tokio::test]
    #[serial]
    async fn import_creates_and_updates_without_resetting_existing_password() {
        let Some(test_db) = crate::test_support::TestDb::try_new().await else {
            return;
        };
        let seed = crate::test_support::seed_base_data(&test_db.pool).await;
        sqlx::query("INSERT INTO kelas (nama, tingkat) VALUES ('8.B', 8)")
            .execute(&test_db.pool)
            .await
            .unwrap();
        let kelas_baru: i64 = sqlx::query_scalar("SELECT CAST(LAST_INSERT_ID() AS SIGNED)")
            .fetch_one(&test_db.pool)
            .await
            .unwrap();
        let old_password: String = sqlx::query_scalar("SELECT password FROM users WHERE nis = ?")
            .bind(&seed.nis)
            .fetch_one(&test_db.pool)
            .await
            .unwrap();

        let rows = vec![
            ImportStudentRow {
                row_number: 2,
                name: "Nama Siswa Diperbarui".to_string(),
                nis: seed.nis.parse().unwrap(),
                kelas_id: kelas_baru,
                tingkat: 8,
                tahun: seed.tahun.clone(),
            },
            ImportStudentRow {
                row_number: 3,
                name: "Siswa Baru".to_string(),
                nis: 2_699_001,
                kelas_id: seed.kelas_id,
                tingkat: 7,
                tahun: seed.tahun.clone(),
            },
        ];

        let summary = execute_import(&test_db.pool, &rows).await.unwrap();
        assert_eq!(summary.total_rows, 2);
        assert_eq!(summary.user_baru, 1);
        assert_eq!(summary.user_diperbarui, 1);
        assert_eq!(summary.siswa_baru, 1);
        assert_eq!(summary.siswa_diperbarui, 1);

        let existing: (String, String) =
            sqlx::query_as("SELECT name, password FROM users WHERE nis = ?")
                .bind(&seed.nis)
                .fetch_one(&test_db.pool)
                .await
                .unwrap();
        assert_eq!(existing.0, "Nama Siswa Diperbarui");
        assert_eq!(existing.1, old_password);

        let new_user: (i64, String) =
            sqlx::query_as("SELECT CAST(id AS SIGNED), password FROM users WHERE nis = 2699001")
                .fetch_one(&test_db.pool)
                .await
                .unwrap();
        assert!(verify("12345678", &new_user.1).unwrap());
        let role_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM model_has_roles m
            JOIN roles r ON r.id = m.role_id
            WHERE m.model_id = ? AND m.model_type = 'App\\Models\\User' AND r.name = 'Siswa'
            "#,
        )
        .bind(new_user.0)
        .fetch_one(&test_db.pool)
        .await
        .unwrap();
        assert_eq!(role_count, 1);

        let existing_student: (i64, i32, String) = sqlx::query_as(
            "SELECT CAST(kelas_id AS SIGNED), CAST(tingkat AS SIGNED), tahun FROM siswas WHERE nis = ? AND tahun = ?",
        )
        .bind(&seed.nis)
        .bind(&seed.tahun)
        .fetch_one(&test_db.pool)
        .await
        .unwrap();
        assert_eq!(existing_student, (kelas_baru, 8, seed.tahun.clone()));

        let second_summary = execute_import(&test_db.pool, &rows).await.unwrap();
        assert_eq!(second_summary.user_baru, 0);
        assert_eq!(second_summary.user_diperbarui, 2);
        assert_eq!(second_summary.siswa_baru, 0);
        assert_eq!(second_summary.siswa_diperbarui, 2);
        let student_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM siswas WHERE nis = 2699001 AND tahun = ?")
                .bind(&seed.tahun)
                .fetch_one(&test_db.pool)
                .await
                .unwrap();
        assert_eq!(student_count, 1);

        test_db.teardown().await;
    }

    #[tokio::test]
    #[serial]
    async fn student_failure_rolls_back_user_change() {
        let Some(test_db) = crate::test_support::TestDb::try_new().await else {
            return;
        };
        let seed = crate::test_support::seed_base_data(&test_db.pool).await;
        sqlx::query(
            r#"
            CREATE TRIGGER reject_student_insert
            BEFORE INSERT ON siswas
            FOR EACH ROW
            SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT = 'forced test failure'
            "#,
        )
        .execute(&test_db.pool)
        .await
        .unwrap();

        let result = execute_import(
            &test_db.pool,
            &[ImportStudentRow {
                row_number: 2,
                name: "Harus Rollback".to_string(),
                nis: 2_699_099,
                kelas_id: seed.kelas_id,
                tingkat: 7,
                tahun: seed.tahun,
            }],
        )
        .await;
        assert!(result.is_err());
        let user_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE nis = 2699099")
            .fetch_one(&test_db.pool)
            .await
            .unwrap();
        assert_eq!(user_count, 0);

        test_db.teardown().await;
    }

    #[tokio::test]
    #[serial]
    async fn upload_data_siswa_routes_are_guru_only() {
        let Some(test_db) = crate::test_support::TestDb::try_new().await else {
            return;
        };
        let tera = crate::test_support::build_test_tera();
        let guru_app = Router::new()
            .merge(guru_only_routes())
            .layer(Extension(test_db.pool.clone()))
            .layer(Extension(tera.clone()))
            .layer(Extension(AuthUser {
                id: 1,
                nis: None,
                roles: vec![Role::Guru],
                name: "Guru".to_string(),
                foto: None,
            }));
        let guru_response = guru_app
            .oneshot(
                Request::builder()
                    .uri("/upload-data-siswa")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(guru_response.status(), StatusCode::OK);

        let admin_app = Router::new()
            .merge(guru_only_routes())
            .layer(Extension(test_db.pool.clone()))
            .layer(Extension(tera))
            .layer(Extension(AuthUser {
                id: 2,
                nis: None,
                roles: vec![Role::Admin],
                name: "Admin".to_string(),
                foto: None,
            }));
        let admin_response = admin_app
            .oneshot(
                Request::builder()
                    .uri("/upload-data-siswa")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(admin_response.status(), StatusCode::FORBIDDEN);

        test_db.teardown().await;
    }
}
