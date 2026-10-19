use std::collections::HashMap;

use axum::{
    extract::Query,
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Redirect},
};
use serde::Serialize;
use sqlx::MySqlPool;
use tera::Context;

use crate::controllers::guru::absensi_kelas::Htmx;
use crate::utils::{
    list_kelas::list_kelas, page_context::PageContext, render::render, tahun::data_tahun,
};

use super::{MapelOption, NilaiFilter, NilaiKelasRow, fetch_mapel_options};

#[derive(sqlx::FromRow)]
struct ExportMetaRow {
    kelas: String,
    mata_pelajaran: String,
    ujian_title: Option<String>,
    ujian_description: Option<String>,
}

#[derive(Clone, Serialize, sqlx::FromRow)]
struct UjianNilaiOption {
    id: i64,
    title: String,
}

#[derive(Clone, sqlx::FromRow)]
struct ExamQuestionRow {
    soal_id: i64,
    urutan: i32,
    kategori: String,
    opsi_a: Option<String>,
    opsi_b: Option<String>,
    opsi_c: Option<String>,
    opsi_d: Option<String>,
}

#[derive(Clone, sqlx::FromRow)]
struct ExamAnswerRow {
    nis: String,
    soal_id: i64,
    pilihan: Option<String>,
    jawaban_uraian: Option<String>,
    is_benar: i8,
}

struct AnswerCell {
    text: Option<String>,
    correct: Option<f64>,
}

#[derive(Serialize)]
struct UjianOptionsData {
    list_ujian: Vec<UjianNilaiOption>,
    ujian_id: i64,
}

#[derive(Serialize)]
struct NilaiKelasPageData {
    tahun: String,
    list_kelas: Vec<crate::utils::list_kelas::ListKelas>,
    list_mapel: Vec<MapelOption>,
    list_ujian: Vec<UjianNilaiOption>,
    kelas_id: i64,
    mata_pelajaran_id: i64,
    ujian_id: i64,
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
    let mata_pelajaran_id = filter.mata_pelajaran_id.unwrap_or(0);
    let list_ujian = fetch_ujian_options(&db, mata_pelajaran_id, &tahun).await;
    let ujian_id = selected_ujian_id(filter.ujian_id, &list_ujian).unwrap_or(0);

    let data = NilaiKelasPageData {
        tahun,
        list_kelas,
        list_mapel,
        list_ujian,
        kelas_id: filter.kelas_id.unwrap_or(0),
        mata_pelajaran_id,
        ujian_id,
        require_kelas: true,
    };

    render(
        &ctx,
        "guru/ujian/nilai_kelas_mapel.html",
        "Nilai Kelas Mapel",
        data,
    )
}

pub async fn nilai_ujian_options(
    ctx: PageContext,
    Query(filter): Query<NilaiFilter>,
    axum::Extension(db): axum::Extension<MySqlPool>,
) -> Html<String> {
    let tahun = filter.tahun.clone().unwrap_or_else(data_tahun);
    let list_ujian = fetch_ujian_options(&db, filter.mata_pelajaran_id.unwrap_or(0), &tahun).await;
    let ujian_id = selected_ujian_id(filter.ujian_id, &list_ujian).unwrap_or(0);
    let mut tera_ctx = Context::new();
    tera_ctx.insert(
        "data",
        &UjianOptionsData {
            list_ujian,
            ujian_id,
        },
    );

    Html(
        ctx.tera
            .render("guru/ujian/_nilai_ujian_options.html", &tera_ctx)
            .unwrap_or_default(),
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
            "/hasil-nilai?kelas_id={}&mata_pelajaran_id={}&ujian_id={}&tahun={}",
            filter.kelas_id.unwrap_or(0),
            filter.mata_pelajaran_id.unwrap_or(0),
            filter.ujian_id.unwrap_or(0),
            filter.tahun.clone().unwrap_or_else(data_tahun)
        );
        return Redirect::to(&url).into_response();
    }

    let kelas_id = filter.kelas_id.unwrap_or(0);
    let mata_pelajaran_id = filter.mata_pelajaran_id.unwrap_or(0);
    let tahun = filter.tahun.clone().unwrap_or_else(data_tahun);
    let ujian_id = match validated_ujian_id(&db, filter.ujian_id, mata_pelajaran_id, &tahun).await {
        Ok(ujian_id) => ujian_id,
        Err(StatusCode::BAD_REQUEST) => None,
        Err(status) => return status.into_response(),
    };
    let rows = fetch_nilai_kelas_rows(&db, kelas_id, mata_pelajaran_id, &tahun, ujian_id).await;

    let mut tera_ctx = Context::new();
    tera_ctx.insert("rows", &rows);
    tera_ctx.insert("kelas_id", &kelas_id);
    tera_ctx.insert("mata_pelajaran_id", &mata_pelajaran_id);
    tera_ctx.insert("ujian_id", &ujian_id.unwrap_or(0));
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
    let ujian_id = match validated_ujian_id(&db, filter.ujian_id, mata_pelajaran_id, &tahun).await {
        Ok(ujian_id) => ujian_id,
        Err(status) => return status.into_response(),
    };
    let rows = fetch_nilai_kelas_rows(&db, kelas_id, mata_pelajaran_id, &tahun, ujian_id).await;
    let meta = fetch_export_meta(&db, kelas_id, mata_pelajaran_id, ujian_id).await;
    let body = build_excel_xml(&rows, &meta, &tahun);
    excel_response(body, export_filename("hasil-nilai", &meta, &tahun))
}

pub async fn nilai_kelas_mapel_export_jawaban(
    Query(filter): Query<NilaiFilter>,
    axum::Extension(db): axum::Extension<MySqlPool>,
) -> axum::response::Response {
    let kelas_id = filter.kelas_id.unwrap_or(0);
    let mata_pelajaran_id = filter.mata_pelajaran_id.unwrap_or(0);
    if kelas_id <= 0 || mata_pelajaran_id <= 0 {
        return (
            StatusCode::BAD_REQUEST,
            "kelas dan mata pelajaran wajib dipilih",
        )
            .into_response();
    }

    let tahun = filter.tahun.clone().unwrap_or_else(data_tahun);
    let ujian_id = match validated_ujian_id(&db, filter.ujian_id, mata_pelajaran_id, &tahun).await {
        Ok(ujian_id) => ujian_id,
        Err(status) => return status.into_response(),
    };
    let Some(ujian_id) = ujian_id else {
        return (
            StatusCode::BAD_REQUEST,
            "ujian wajib dipilih dan harus sesuai tahun serta mata pelajaran",
        )
            .into_response();
    };

    let rows =
        fetch_nilai_kelas_rows(&db, kelas_id, mata_pelajaran_id, &tahun, Some(ujian_id)).await;
    let meta = fetch_export_meta(&db, kelas_id, mata_pelajaran_id, Some(ujian_id)).await;
    let questions = match fetch_exam_questions(&db, ujian_id).await {
        Ok(rows) => rows,
        Err(error) => {
            eprintln!("ERROR fetch exam questions for export: {error:?}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    let answers = match fetch_exam_answers(&db, ujian_id).await {
        Ok(rows) => rows,
        Err(error) => {
            eprintln!("ERROR fetch exam answers for export: {error:?}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let answer_map = answers
        .into_iter()
        .map(|answer| ((answer.nis.clone(), answer.soal_id), answer))
        .collect::<HashMap<_, _>>();
    let body = build_excel_with_answers(&rows, &questions, &answer_map, &meta, &tahun);
    excel_response(body, export_filename("hasil-nilai-jawaban", &meta, &tahun))
}

async fn fetch_ujian_options(
    db: &MySqlPool,
    mata_pelajaran_id: i64,
    tahun: &str,
) -> Vec<UjianNilaiOption> {
    if mata_pelajaran_id <= 0 {
        return Vec::new();
    }

    sqlx::query_as::<_, UjianNilaiOption>(
        r#"
            SELECT id, title
            FROM ujians
            WHERE mata_pelajaran_id = ? AND tahun = ?
            ORDER BY tanggal DESC, id DESC
        "#,
    )
    .bind(mata_pelajaran_id)
    .bind(tahun)
    .fetch_all(db)
    .await
    .unwrap_or_else(|error| {
        eprintln!("ERROR fetch ujian options: {error:?}");
        Vec::new()
    })
}

fn selected_ujian_id(requested: Option<i64>, options: &[UjianNilaiOption]) -> Option<i64> {
    requested.filter(|id| *id > 0 && options.iter().any(|option| option.id == *id))
}

async fn validated_ujian_id(
    db: &MySqlPool,
    requested: Option<i64>,
    mata_pelajaran_id: i64,
    tahun: &str,
) -> Result<Option<i64>, StatusCode> {
    let Some(requested) = requested.filter(|id| *id > 0) else {
        return Ok(None);
    };
    let selected = sqlx::query_scalar::<_, i64>(
        "SELECT id FROM ujians WHERE id = ? AND mata_pelajaran_id = ? AND tahun = ? LIMIT 1",
    )
    .bind(requested)
    .bind(mata_pelajaran_id)
    .bind(tahun)
    .fetch_optional(db)
    .await
    .map_err(|error| {
        eprintln!("ERROR validate ujian filter: {error:?}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    selected.map(Some).ok_or(StatusCode::BAD_REQUEST)
}

async fn fetch_nilai_kelas_rows(
    db: &MySqlPool,
    kelas_id: i64,
    mata_pelajaran_id: i64,
    tahun: &str,
    ujian_id: Option<i64>,
) -> Vec<NilaiKelasRow> {
    if kelas_id <= 0 {
        return Vec::new();
    }

    let student_sql = if ujian_id.is_some() {
        r#"
            SELECT TRIM(s.nis) AS nis, u.name AS nama, k.nama AS kelas,
                NULL AS total_benar, NULL AS total_salah, NULL AS nilai_pg,
                NULL AS nilai_uraian, NULL AS total_nilai
            FROM siswas s
            JOIN kelas k ON k.id = s.kelas_id
            LEFT JOIN user_siswa u ON u.nis = s.nis
            WHERE s.kelas_id = ? AND s.tahun = ?
            ORDER BY u.name IS NULL, u.name ASC, s.nis ASC
        "#
    } else {
        r#"
            SELECT TRIM(s.nis) AS nis, u.name AS nama, k.nama AS kelas,
                CAST(0 AS DOUBLE) AS total_benar, CAST(0 AS DOUBLE) AS total_salah,
                CAST(0 AS DOUBLE) AS nilai_pg, CAST(0 AS DOUBLE) AS nilai_uraian,
                CAST(0 AS DOUBLE) AS total_nilai
            FROM siswas s
            JOIN kelas k ON k.id = s.kelas_id
            LEFT JOIN user_siswa u ON u.nis = s.nis
            WHERE s.kelas_id = ? AND s.tahun = ?
            ORDER BY u.name IS NULL, u.name ASC, s.nis ASC
        "#
    };

    let mut rows = match sqlx::query_as::<_, NilaiKelasRow>(student_sql)
        .bind(kelas_id)
        .bind(tahun)
        .fetch_all(db)
        .await
    {
        Ok(rows) => rows,
        Err(error) => {
            eprintln!("ERROR fetch students for nilai export: {error:?}");
            return Vec::new();
        }
    };

    if mata_pelajaran_id <= 0 {
        return rows;
    }

    let fetched_hasil = if let Some(ujian_id) = ujian_id {
        sqlx::query_as::<_, (String, f64, f64, f64, f64, f64)>(
            r#"
                SELECT TRIM(nis) AS nis,
                    CAST(COALESCE(total_benar, 0) AS DOUBLE),
                    CAST(COALESCE(total_salah, 0) AS DOUBLE),
                    CAST(COALESCE(total_pg, 0) AS DOUBLE),
                    CAST(COALESCE(total_uraian, 0) AS DOUBLE),
                    CAST(COALESCE(total_nilai, 0) AS DOUBLE)
                FROM hasil_nilais
                WHERE mata_pelajaran_id = ? AND tahun = ? AND ujian_id = ?
            "#,
        )
        .bind(mata_pelajaran_id)
        .bind(tahun)
        .bind(ujian_id)
        .fetch_all(db)
        .await
    } else {
        sqlx::query_as::<_, (String, f64, f64, f64, f64, f64)>(
            r#"
                SELECT TRIM(nis) AS nis,
                    CAST(COALESCE(total_benar, 0) AS DOUBLE),
                    CAST(COALESCE(total_salah, 0) AS DOUBLE),
                    CAST(COALESCE(total_pg, 0) AS DOUBLE),
                    CAST(COALESCE(total_uraian, 0) AS DOUBLE),
                    CAST(COALESCE(total_nilai, 0) AS DOUBLE)
                FROM hasil_nilais
                WHERE mata_pelajaran_id = ? AND tahun = ?
            "#,
        )
        .bind(mata_pelajaran_id)
        .bind(tahun)
        .fetch_all(db)
        .await
    };

    match fetched_hasil {
        Ok(fetched_hasil) => {
            let nilai_map: HashMap<String, (f64, f64, f64, f64, f64)> = fetched_hasil
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
        Err(error) => eprintln!("ERROR fetch hasil nilai: {error:?}"),
    }
    rows
}

async fn fetch_exam_questions(
    db: &MySqlPool,
    ujian_id: i64,
) -> Result<Vec<ExamQuestionRow>, sqlx::Error> {
    sqlx::query_as::<_, ExamQuestionRow>(
        r#"
            SELECT s.id AS soal_id, COALESCE(us.urutan, 0) AS urutan,
                COALESCE(s.kategori, 'Pilihan Ganda') AS kategori,
                s.opsi_a, s.opsi_b, s.opsi_c, s.opsi_d
            FROM ujian_soals us
            JOIN soals s ON s.id = us.soal_id
            WHERE us.ujian_id = ?
            ORDER BY us.urutan ASC, us.soal_id ASC
        "#,
    )
    .bind(ujian_id)
    .fetch_all(db)
    .await
}

async fn fetch_exam_answers(
    db: &MySqlPool,
    ujian_id: i64,
) -> Result<Vec<ExamAnswerRow>, sqlx::Error> {
    sqlx::query_as::<_, ExamAnswerRow>(
        r#"
            SELECT TRIM(nis) AS nis, soal_id, pilihan, jawaban_uraian,
                CAST(is_benar AS SIGNED) AS is_benar
            FROM ujian_jawabans
            WHERE ujian_id = ?
        "#,
    )
    .bind(ujian_id)
    .fetch_all(db)
    .await
}

fn build_excel_xml(rows: &[NilaiKelasRow], meta: &ExportMetaRow, tahun: &str) -> String {
    let mut xml = excel_xml_start("Nilai");
    push_export_metadata(&mut xml, meta, tahun, "REKAP HASIL NILAI UJIAN");
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
        push_fixed_cells(&mut xml, index, row, false);
    }
    finish_excel_xml(&mut xml);
    xml
}

fn build_excel_with_answers(
    rows: &[NilaiKelasRow],
    questions: &[ExamQuestionRow],
    answers: &HashMap<(String, i64), ExamAnswerRow>,
    meta: &ExportMetaRow,
    tahun: &str,
) -> String {
    let mut xml = excel_xml_start("Nilai dan Jawaban");
    push_export_metadata(&mut xml, meta, tahun, "REKAP HASIL NILAI DAN JAWABAN UJIAN");
    let mut headers = vec![
        "No".to_string(),
        "NIS".to_string(),
        "Nama".to_string(),
        "Kelas".to_string(),
        "Benar".to_string(),
        "Salah".to_string(),
        "Nilai PG".to_string(),
        "Nilai Uraian".to_string(),
        "Total Nilai".to_string(),
    ];
    for question in questions {
        headers.push(format!("Soal {} - Jawaban", question.urutan));
        headers.push(format!("Soal {} - Benar", question.urutan));
    }
    let header_refs = headers.iter().map(String::as_str).collect::<Vec<_>>();
    push_string_row(&mut xml, &header_refs);

    for (index, row) in rows.iter().enumerate() {
        xml.push_str("   <Row>");
        push_fixed_cell_values(&mut xml, index, row, true);
        for question in questions {
            let answer = answers.get(&(row.nis.clone(), question.soal_id));
            let cell = answer_cell(question, answer);
            match cell.text {
                Some(text) => push_string_cell(&mut xml, &text),
                None => push_empty_cell(&mut xml),
            }
            push_number_or_blank_cell(&mut xml, cell.correct);
        }
        xml.push_str("</Row>\n");
    }
    finish_excel_xml(&mut xml);
    xml
}

fn answer_cell(question: &ExamQuestionRow, answer: Option<&ExamAnswerRow>) -> AnswerCell {
    let Some(answer) = answer else {
        return AnswerCell {
            text: None,
            correct: None,
        };
    };

    if question.kategori == "Uraian" {
        return AnswerCell {
            text: answer
                .jawaban_uraian
                .as_deref()
                .filter(|text| !text.trim().is_empty())
                .map(ToOwned::to_owned),
            correct: None,
        };
    }

    let choices = answer
        .pilihan
        .as_deref()
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|choice| !choice.is_empty())
        .collect::<Vec<_>>();
    if choices.is_empty() {
        return AnswerCell {
            text: None,
            correct: None,
        };
    }

    let text = if question.kategori == "Benar/Salah" {
        question
            .options()
            .into_iter()
            .zip(choices.iter())
            .map(|(statement, choice)| {
                format!("{} — {}", plain_text(statement), capitalize_answer(choice))
            })
            .collect::<Vec<_>>()
            .join("; ")
    } else {
        choices
            .iter()
            .map(|choice| {
                question
                    .option_text(choice)
                    .map(plain_text)
                    .unwrap_or_else(|| choice.to_uppercase())
            })
            .collect::<Vec<_>>()
            .join("; ")
    };

    AnswerCell {
        text: (!text.is_empty()).then_some(text),
        correct: Some(if answer.is_benar != 0 { 1.0 } else { 0.0 }),
    }
}

impl ExamQuestionRow {
    fn options(&self) -> Vec<&str> {
        [
            self.opsi_a.as_deref(),
            self.opsi_b.as_deref(),
            self.opsi_c.as_deref(),
            self.opsi_d.as_deref(),
        ]
        .into_iter()
        .flatten()
        .filter(|option| !option.trim().is_empty())
        .collect()
    }

    fn option_text(&self, choice: &str) -> Option<&str> {
        match choice.trim().to_ascii_lowercase().as_str() {
            "a" => self.opsi_a.as_deref(),
            "b" => self.opsi_b.as_deref(),
            "c" => self.opsi_c.as_deref(),
            "d" => self.opsi_d.as_deref(),
            _ => None,
        }
    }
}

fn excel_xml_start(sheet_name: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<?mso-application progid="Excel.Sheet"?>
<Workbook xmlns="urn:schemas-microsoft-com:office:spreadsheet"
 xmlns:o="urn:schemas-microsoft-com:office:office"
 xmlns:x="urn:schemas-microsoft-com:office:excel"
 xmlns:ss="urn:schemas-microsoft-com:office:spreadsheet">
 <Worksheet ss:Name="{}">
  <Table>
"#,
        escape_xml(sheet_name)
    )
}

fn finish_excel_xml(xml: &mut String) {
    xml.push_str("  </Table>\n </Worksheet>\n</Workbook>\n");
}

fn push_export_metadata(xml: &mut String, meta: &ExportMetaRow, tahun: &str, title: &str) {
    push_string_row(xml, &[title]);
    push_string_row(xml, &[&format!("Tahun: {tahun}")]);
    push_string_row(xml, &[&format!("Kelas: {}", meta.kelas)]);
    push_string_row(xml, &[&format!("Mata Pelajaran: {}", meta.mata_pelajaran)]);
    if let Some(ujian_title) = meta.ujian_title.as_deref() {
        push_string_row(xml, &[&format!("Judul Ujian: {ujian_title}")]);
        push_string_row(
            xml,
            &[&format!(
                "Deskripsi: {}",
                meta.ujian_description.as_deref().unwrap_or("-")
            )],
        );
    } else {
        push_string_row(xml, &["Ujian: Rekap seluruh ujian"]);
    }
    push_string_row(xml, &[]);
}

fn push_fixed_cells(xml: &mut String, index: usize, row: &NilaiKelasRow, blank_missing: bool) {
    xml.push_str("   <Row>");
    push_fixed_cell_values(xml, index, row, blank_missing);
    xml.push_str("</Row>\n");
}

fn push_fixed_cell_values(
    xml: &mut String,
    index: usize,
    row: &NilaiKelasRow,
    blank_missing: bool,
) {
    push_number_cell(xml, (index + 1) as f64);
    push_string_cell(xml, &row.nis);
    let fallback_name = format!("NIS {}", row.nis);
    push_string_cell(xml, row.nama.as_deref().unwrap_or(&fallback_name));
    push_string_cell(xml, row.kelas.as_deref().unwrap_or("-"));
    for value in [
        row.total_benar,
        row.total_salah,
        row.nilai_pg,
        row.nilai_uraian,
        row.total_nilai,
    ] {
        if blank_missing {
            push_number_or_blank_cell(xml, value);
        } else {
            push_number_or_empty_cell(xml, value);
        }
    }
}

fn push_string_row(xml: &mut String, values: &[&str]) {
    xml.push_str("   <Row>");
    if values.is_empty() {
        push_empty_cell(xml);
    } else {
        for value in values {
            push_string_cell(xml, value);
        }
    }
    xml.push_str("</Row>\n");
}

fn push_string_cell(xml: &mut String, value: &str) {
    let safe = safe_excel_text(value);
    xml.push_str(&format!(
        r#"<Cell><Data ss:Type="String">{}</Data></Cell>"#,
        escape_xml(&safe)
    ));
}

fn push_number_cell(xml: &mut String, value: f64) {
    xml.push_str(&format!(
        r#"<Cell><Data ss:Type="Number">{value}</Data></Cell>"#
    ));
}

fn push_empty_cell(xml: &mut String) {
    xml.push_str("<Cell/>");
}

fn push_number_or_empty_cell(xml: &mut String, value: Option<f64>) {
    match value {
        Some(number) => push_number_cell(xml, number),
        None => push_string_cell(xml, "-"),
    }
}

fn push_number_or_blank_cell(xml: &mut String, value: Option<f64>) {
    match value {
        Some(number) => push_number_cell(xml, number),
        None => push_empty_cell(xml),
    }
}

fn safe_excel_text(value: &str) -> String {
    if value.trim_start().starts_with(['=', '+', '-', '@']) {
        format!("'{value}")
    } else {
        value.to_string()
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

fn plain_text(value: &str) -> String {
    let mut text = String::new();
    let mut in_tag = false;
    for character in value.chars() {
        match character {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                if !text.ends_with(' ') {
                    text.push(' ');
                }
            }
            _ if !in_tag => text.push(character),
            _ => {}
        }
    }
    text.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn capitalize_answer(value: &str) -> String {
    let value = value.trim();
    let mut characters = value.chars();
    match characters.next() {
        Some(first) => format!("{}{}", first.to_uppercase(), characters.as_str()),
        None => String::new(),
    }
}

async fn fetch_export_meta(
    db: &MySqlPool,
    kelas_id: i64,
    mata_pelajaran_id: i64,
    ujian_id: Option<i64>,
) -> ExportMetaRow {
    let kelas = sqlx::query_scalar::<_, String>("SELECT nama FROM kelas WHERE id = ? LIMIT 1")
        .bind(kelas_id)
        .fetch_optional(db)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| format!("Kelas {kelas_id}"));

    let mata_pelajaran = if mata_pelajaran_id > 0 {
        sqlx::query_scalar::<_, String>("SELECT nama FROM mata_pelajarans WHERE id = ? LIMIT 1")
            .bind(mata_pelajaran_id)
            .fetch_optional(db)
            .await
            .ok()
            .flatten()
            .unwrap_or_else(|| format!("Mapel {mata_pelajaran_id}"))
    } else {
        "Semua Mata Pelajaran".to_string()
    };

    let ujian = if let Some(ujian_id) = ujian_id {
        sqlx::query_as::<_, (String, Option<String>)>(
            "SELECT title, description FROM ujians WHERE id = ? LIMIT 1",
        )
        .bind(ujian_id)
        .fetch_optional(db)
        .await
        .ok()
        .flatten()
    } else {
        None
    };

    ExportMetaRow {
        kelas,
        mata_pelajaran,
        ujian_title: ujian.as_ref().map(|(title, _)| title.clone()),
        ujian_description: ujian.and_then(|(_, description)| description),
    }
}

fn excel_response(body: String, filename: String) -> axum::response::Response {
    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        HeaderValue::from_static("application/vnd.ms-excel; charset=utf-8"),
    );
    headers.insert(
        axum::http::header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"{filename}\"")).unwrap(),
    );
    (headers, body).into_response()
}

fn export_filename(prefix: &str, meta: &ExportMetaRow, tahun: &str) -> String {
    let mut parts = vec![
        prefix.to_string(),
        sanitize_filename_part(&meta.kelas),
        sanitize_filename_part(&meta.mata_pelajaran),
    ];
    if let Some(title) = meta.ujian_title.as_deref() {
        parts.push(sanitize_filename_part(title));
    }
    parts.push(sanitize_filename_part(tahun));
    format!("{}.xls", parts.join("-"))
}

fn sanitize_filename_part(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn question(kategori: &str, urutan: i32) -> ExamQuestionRow {
        ExamQuestionRow {
            soal_id: i64::from(urutan),
            urutan,
            kategori: kategori.to_string(),
            opsi_a: Some("<p>Opsi &amp; A</p>".to_string()),
            opsi_b: Some("Opsi B".to_string()),
            opsi_c: Some("Pernyataan C".to_string()),
            opsi_d: None,
        }
    }

    fn answer(pilihan: Option<&str>, uraian: Option<&str>, benar: i8) -> ExamAnswerRow {
        ExamAnswerRow {
            nis: "00123".to_string(),
            soal_id: 1,
            pilihan: pilihan.map(ToOwned::to_owned),
            jawaban_uraian: uraian.map(ToOwned::to_owned),
            is_benar: benar,
        }
    }

    fn student() -> NilaiKelasRow {
        NilaiKelasRow {
            nis: "00123".to_string(),
            nama: Some("Budi".to_string()),
            kelas: Some("8A".to_string()),
            total_benar: Some(1.0),
            total_salah: Some(0.0),
            nilai_pg: Some(75.0),
            nilai_uraian: None,
            total_nilai: Some(75.0),
        }
    }

    fn meta(selected: bool) -> ExportMetaRow {
        ExportMetaRow {
            kelas: "8A".to_string(),
            mata_pelajaran: "Informatika".to_string(),
            ujian_title: selected.then(|| "Ujian Data".to_string()),
            ujian_description: selected.then(|| "Deskripsi & evaluasi".to_string()),
        }
    }

    #[test]
    fn legacy_export_keeps_fixed_columns_and_aggregate_label() {
        let xml = build_excel_xml(&[student()], &meta(false), "2026 / 2027");
        assert!(xml.contains("Ujian: Rekap seluruh ujian"));
        assert!(xml.contains("Total Nilai"));
        assert!(!xml.contains("Soal 1 - Jawaban"));
        assert!(xml.contains(">00123<"));
    }

    #[test]
    fn selected_exam_metadata_is_exported_and_escaped() {
        let xml = build_excel_xml(&[student()], &meta(true), "2026 / 2027");
        assert!(xml.contains("Judul Ujian: Ujian Data"));
        assert!(xml.contains("Deskripsi: Deskripsi &amp; evaluasi"));
    }

    #[test]
    fn maps_regular_and_complex_choices_and_stored_correctness() {
        let regular_answer = answer(Some("a"), None, 1);
        let regular = answer_cell(&question("Pilihan Ganda", 1), Some(&regular_answer));
        assert_eq!(regular.text.as_deref(), Some("Opsi & A"));
        assert_eq!(regular.correct, Some(1.0));

        let wrong_answer = answer(Some("b"), None, 0);
        let wrong = answer_cell(&question("Pilihan Ganda", 1), Some(&wrong_answer));
        assert_eq!(wrong.text.as_deref(), Some("Opsi B"));
        assert_eq!(wrong.correct, Some(0.0));

        let complex_answer = answer(Some("a,c"), None, 0);
        let complex = answer_cell(
            &question("Pilihan Ganda Kompleks MCMA", 1),
            Some(&complex_answer),
        );
        assert_eq!(complex.text.as_deref(), Some("Opsi & A; Pernyataan C"));
        assert_eq!(complex.correct, Some(0.0));
    }

    #[test]
    fn maps_true_false_statements_in_stored_order() {
        let stored = answer(Some("benar,salah,benar"), None, 0);
        let cell = answer_cell(&question("Benar/Salah", 1), Some(&stored));
        assert_eq!(
            cell.text.as_deref(),
            Some("Opsi & A — Benar; Opsi B — Salah; Pernyataan C — Benar")
        );
        assert_eq!(cell.correct, Some(0.0));
    }

    #[test]
    fn essay_and_unanswered_correctness_stay_blank() {
        let essay_answer = answer(None, Some("Jawaban panjang <aman> & lengkap"), 0);
        let essay = answer_cell(&question("Uraian", 1), Some(&essay_answer));
        assert_eq!(
            essay.text.as_deref(),
            Some("Jawaban panjang <aman> & lengkap")
        );
        assert_eq!(essay.correct, None);

        let unanswered = answer_cell(&question("Pilihan Ganda", 1), None);
        assert!(unanswered.text.is_none());
        assert!(unanswered.correct.is_none());
    }

    #[test]
    fn answer_export_follows_question_order_and_blocks_formula_injection() {
        let mut first = question("Uraian", 2);
        first.soal_id = 10;
        let mut second = question("Pilihan Ganda", 5);
        second.soal_id = 20;
        let questions = vec![first, second];
        let mut answers = HashMap::new();
        answers.insert(
            ("00123".to_string(), 10),
            ExamAnswerRow {
                nis: "00123".to_string(),
                soal_id: 10,
                pilihan: None,
                jawaban_uraian: Some("=HYPERLINK(\"x\")".to_string()),
                is_benar: 0,
            },
        );
        let xml = build_excel_with_answers(
            &[student()],
            &questions,
            &answers,
            &meta(true),
            "2026 / 2027",
        );
        assert!(xml.find("Soal 2 - Jawaban").unwrap() < xml.find("Soal 5 - Jawaban").unwrap());
        assert!(xml.contains("&apos;=HYPERLINK(&quot;x&quot;)"));
        assert!(xml.contains("<Cell/>"));
    }

    #[test]
    fn long_answer_is_not_truncated() {
        let long_answer = "isi & khusus ".repeat(1_000);
        let stored = answer(None, Some(&long_answer), 0);
        let cell = answer_cell(&question("Uraian", 1), Some(&stored));
        assert_eq!(cell.text.as_deref(), Some(long_answer.as_str()));
        let mut xml = String::new();
        push_string_cell(&mut xml, cell.text.as_deref().unwrap());
        assert!(xml.len() > long_answer.len());
        assert!(xml.contains("&amp;"));
    }

    #[test]
    fn only_valid_exam_option_can_remain_selected() {
        let options = vec![
            UjianNilaiOption {
                id: 1,
                title: "Satu".to_string(),
            },
            UjianNilaiOption {
                id: 2,
                title: "Dua".to_string(),
            },
        ];
        assert_eq!(selected_ujian_id(Some(2), &options), Some(2));
        assert_eq!(selected_ujian_id(Some(3), &options), None);
    }
}
