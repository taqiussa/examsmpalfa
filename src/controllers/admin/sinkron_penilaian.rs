use std::collections::{HashMap, HashSet};

use axum::{
    Form,
    extract::Query,
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use sqlx::MySqlPool;
use tera::Context;

use crate::{
    controllers::guru::ujian::{
        MapelOption, fetch_mapel_options,
        nilai::{
            UjianNilaiOption, fetch_nilai_kelas_rows, fetch_ujian_options, validated_ujian_id,
        },
    },
    utils::{
        list_kelas::{ListKelas, list_kelas},
        page_context::PageContext,
        render::render,
        semester::data_semester,
        tahun::data_tahun,
    },
};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct SinkronFilter {
    pub tahun: Option<String>,
    pub semester: Option<u8>,
    pub kelas_id: Option<i64>,
    pub mata_pelajaran_id: Option<i64>,
    pub ujian_id: Option<i64>,
    pub kategori_nilai_id: Option<i64>,
    pub jenis_penilaian_id: Option<i64>,
}

#[derive(Serialize)]
struct SinkronPageData {
    tahun: String,
    semester: u8,
    list_kelas: Vec<ListKelas>,
    list_mapel: Vec<MapelOption>,
    list_ujian: Vec<UjianNilaiOption>,
    kelas_id: i64,
    mata_pelajaran_id: i64,
    ujian_id: i64,
    kategori_nilai_id: i64,
    jenis_penilaian_id: i64,
    require_kelas: bool,
}

#[derive(Clone, Serialize, sqlx::FromRow)]
struct ReferenceOption {
    kategori_nilai_id: i64,
    kategori_nilai: String,
    jenis_penilaian_id: i64,
    jenis_penilaian: String,
}

#[derive(Serialize)]
struct ReferenceData {
    categories: Vec<ReferenceCategory>,
    types: Vec<ReferenceType>,
    kategori_nilai_id: i64,
    jenis_penilaian_id: i64,
    tingkat: Option<u8>,
}

#[derive(Serialize)]
struct ReferenceCategory {
    id: i64,
    nama: String,
}

#[derive(Serialize)]
struct ReferenceType {
    id: i64,
    nama: String,
}

#[derive(Serialize)]
struct UjianOptionsData {
    list_ujian: Vec<UjianNilaiOption>,
    ujian_id: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "UPPERCASE")]
enum SyncStatus {
    Insert,
    Update,
    Unchanged,
    Skipped,
    Conflict,
}

#[derive(Clone, Debug, Serialize)]
struct PreviewRow {
    nis: String,
    nis_numeric: Option<u32>,
    nama: Option<String>,
    kelas: Option<String>,
    total_benar: Option<f64>,
    total_salah: Option<f64>,
    nilai_pg: Option<f64>,
    nilai_uraian: Option<f64>,
    total_nilai: Option<f64>,
    nilai_target: Option<i32>,
    nilai_sync: Option<i32>,
    status: SyncStatus,
    alasan: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
struct PreviewSummary {
    insert: usize,
    update: usize,
    unchanged: usize,
    skipped: usize,
    conflict: usize,
    writable: usize,
}

#[derive(Serialize)]
struct PreviewData {
    rows: Vec<PreviewRow>,
    summary: PreviewSummary,
    executed: bool,
}

#[derive(sqlx::FromRow)]
struct ExistingScore {
    nis: u32,
    nilai: Option<i32>,
    record_count: i64,
}

#[derive(Debug)]
struct ValidatedContext {
    tahun: String,
    semester: u8,
    kelas_id: i64,
    tingkat: u8,
    mata_pelajaran_id: i64,
    kategori_nilai_id: i64,
    jenis_penilaian_id: i64,
}

pub async fn sinkron_penilaian_page(
    ctx: PageContext,
    Query(filter): Query<SinkronFilter>,
    axum::Extension(db): axum::Extension<MySqlPool>,
) -> Html<String> {
    let tahun = filter.tahun.clone().unwrap_or_else(data_tahun);
    let semester = filter.semester.unwrap_or_else(|| data_semester() as u8);
    let mata_pelajaran_id = filter.mata_pelajaran_id.unwrap_or(0);
    let list_ujian = fetch_ujian_options(&db, mata_pelajaran_id, &tahun).await;
    let ujian_id = filter
        .ujian_id
        .filter(|id| list_ujian.iter().any(|ujian| ujian.id == *id))
        .unwrap_or(0);

    let data = SinkronPageData {
        tahun,
        semester,
        list_kelas: list_kelas(&db).await.unwrap_or_default(),
        list_mapel: fetch_mapel_options(&db).await,
        list_ujian,
        kelas_id: filter.kelas_id.unwrap_or(0),
        mata_pelajaran_id,
        ujian_id,
        kategori_nilai_id: filter.kategori_nilai_id.unwrap_or(0),
        jenis_penilaian_id: filter.jenis_penilaian_id.unwrap_or(0),
        require_kelas: true,
    };

    render(
        &ctx,
        "admin/sinkron_penilaian.html",
        "Sinkron Penilaian",
        data,
    )
}

pub async fn sinkron_ujian_options(
    ctx: PageContext,
    Query(filter): Query<SinkronFilter>,
    axum::Extension(db): axum::Extension<MySqlPool>,
) -> Html<String> {
    let tahun = filter.tahun.unwrap_or_else(data_tahun);
    let options = fetch_ujian_options(&db, filter.mata_pelajaran_id.unwrap_or(0), &tahun).await;
    let selected = filter
        .ujian_id
        .filter(|id| options.iter().any(|ujian| ujian.id == *id))
        .unwrap_or(0);
    let mut tera_ctx = Context::new();
    tera_ctx.insert(
        "data",
        &UjianOptionsData {
            list_ujian: options,
            ujian_id: selected,
        },
    );
    Html(
        ctx.tera
            .render("admin/_sinkron_ujian_options.html", &tera_ctx)
            .unwrap_or_default(),
    )
}

pub async fn sinkron_reference_options(
    ctx: PageContext,
    Query(filter): Query<SinkronFilter>,
    axum::Extension(db): axum::Extension<MySqlPool>,
) -> Html<String> {
    let tahun = filter.tahun.unwrap_or_else(data_tahun);
    let semester = filter.semester.unwrap_or_else(|| data_semester() as u8);
    let tingkat = fetch_class_level(&db, filter.kelas_id.unwrap_or(0))
        .await
        .ok();
    let references = if let Some(tingkat) = tingkat {
        fetch_references(&db, &tahun, semester, tingkat)
            .await
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let requested_category = filter.kategori_nilai_id.unwrap_or(0);
    let category_is_valid = references
        .iter()
        .any(|item| item.kategori_nilai_id == requested_category);
    let kategori_nilai_id = if category_is_valid {
        requested_category
    } else {
        0
    };
    let requested_type = filter.jenis_penilaian_id.unwrap_or(0);
    let jenis_penilaian_id = references
        .iter()
        .any(|item| {
            item.kategori_nilai_id == kategori_nilai_id && item.jenis_penilaian_id == requested_type
        })
        .then_some(requested_type)
        .unwrap_or(0);

    let mut categories = Vec::new();
    let mut seen_categories = HashSet::new();
    let mut types = Vec::new();
    for reference in references {
        if seen_categories.insert(reference.kategori_nilai_id) {
            categories.push(ReferenceCategory {
                id: reference.kategori_nilai_id,
                nama: reference.kategori_nilai.clone(),
            });
        }
        if reference.kategori_nilai_id == kategori_nilai_id {
            types.push(ReferenceType {
                id: reference.jenis_penilaian_id,
                nama: reference.jenis_penilaian,
            });
        }
    }

    let mut tera_ctx = Context::new();
    tera_ctx.insert(
        "data",
        &ReferenceData {
            categories,
            types,
            kategori_nilai_id,
            jenis_penilaian_id,
            tingkat,
        },
    );
    Html(
        ctx.tera
            .render("admin/_sinkron_reference_options.html", &tera_ctx)
            .unwrap_or_default(),
    )
}

pub async fn sinkron_preview(
    ctx: PageContext,
    Query(filter): Query<SinkronFilter>,
    axum::Extension(db): axum::Extension<MySqlPool>,
) -> Response {
    match build_preview(&db, &filter).await {
        Ok((rows, summary, _)) => render_preview(&ctx, rows, summary, false, None),
        Err((status, message)) => render_preview_error(&ctx, status, &message),
    }
}

pub async fn sinkron_execute(
    ctx: PageContext,
    axum::Extension(db): axum::Extension<MySqlPool>,
    Form(filter): Form<SinkronFilter>,
) -> Response {
    let (mut rows, _, validated) = match build_preview(&db, &filter).await {
        Ok(preview) => preview,
        Err((status, message)) => return render_preview_error(&ctx, status, &message),
    };

    let mut tx = match db.begin().await {
        Ok(tx) => tx,
        Err(error) => {
            eprintln!("ERROR begin sinkron penilaian: {error:?}");
            return render_preview_error(
                &ctx,
                StatusCode::INTERNAL_SERVER_ERROR,
                "Gagal memulai transaksi sinkronisasi.",
            );
        }
    };

    let reference_lock = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT CAST(id AS SIGNED)
        FROM penilaian_rapors
        WHERE tahun = ? AND semester = ? AND tingkat = ?
          AND kategori_nilai_id = ? AND jenis_penilaian_id = ?
        LIMIT 1 FOR UPDATE
        "#,
    )
    .bind(&validated.tahun)
    .bind(validated.semester.to_string())
    .bind(validated.tingkat)
    .bind(validated.kategori_nilai_id)
    .bind(validated.jenis_penilaian_id)
    .fetch_optional(&mut *tx)
    .await;

    if !matches!(reference_lock, Ok(Some(_))) {
        let _ = tx.rollback().await;
        return render_preview_error(
            &ctx,
            StatusCode::CONFLICT,
            "Referensi penilaian berubah. Muat ulang pilihan lalu preview kembali.",
        );
    }

    for row in rows
        .iter_mut()
        .filter(|row| matches!(row.status, SyncStatus::Insert | SyncStatus::Update))
    {
        let result = sqlx::query(
            r#"
            INSERT INTO penilaians
                (nis, tahun, semester, kelas_id, mata_pelajaran_id,
                 kategori_nilai_id, jenis_penilaian_id, nilai, user_id, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, NOW(), NOW())
            ON DUPLICATE KEY UPDATE
                updated_at = IF(NOT (nilai <=> VALUES(nilai)), NOW(), updated_at),
                nilai = VALUES(nilai)
            "#,
        )
        .bind(row.nis_numeric)
        .bind(&validated.tahun)
        .bind(validated.semester.to_string())
        .bind(validated.kelas_id)
        .bind(validated.mata_pelajaran_id)
        .bind(validated.kategori_nilai_id)
        .bind(validated.jenis_penilaian_id)
        .bind(row.nilai_sync)
        .bind(ctx.user.id)
        .execute(&mut *tx)
        .await;

        match result {
            Ok(result) => {
                row.status = match result.rows_affected() {
                    1 => SyncStatus::Insert,
                    2 => SyncStatus::Update,
                    _ => SyncStatus::Unchanged,
                };
                row.nilai_target = row.nilai_sync;
            }
            Err(error) => {
                eprintln!("ERROR write sinkron penilaian: {error:?}");
                let _ = tx.rollback().await;
                return render_preview_error(
                    &ctx,
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Sinkronisasi gagal; seluruh perubahan batch dibatalkan.",
                );
            }
        }
    }

    if let Err(error) = tx.commit().await {
        eprintln!("ERROR commit sinkron penilaian: {error:?}");
        return render_preview_error(
            &ctx,
            StatusCode::INTERNAL_SERVER_ERROR,
            "Sinkronisasi gagal disimpan; transaksi dibatalkan.",
        );
    }

    let summary = summarize(&rows);
    render_preview(
        &ctx,
        rows,
        summary.clone(),
        true,
        Some(format!(
            "Sinkronisasi selesai: {} INSERT, {} UPDATE, {} tidak berubah.",
            summary.insert, summary.update, summary.unchanged
        )),
    )
}

async fn build_preview(
    db: &MySqlPool,
    filter: &SinkronFilter,
) -> Result<(Vec<PreviewRow>, PreviewSummary, ValidatedContext), (StatusCode, String)> {
    let tahun = filter
        .tahun
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_bad_request("Tahun akademik wajib dipilih.")?
        .to_string();
    let semester = filter
        .semester
        .filter(|value| matches!(value, 1 | 2))
        .ok_or_bad_request("Semester tidak valid.")?;
    let kelas_id = positive(filter.kelas_id, "Kelas wajib dipilih.")?;
    let mata_pelajaran_id = positive(filter.mata_pelajaran_id, "Mata pelajaran wajib dipilih.")?;
    let ujian_id = positive(filter.ujian_id, "Ujian wajib dipilih.")?;
    let kategori_nilai_id = positive(filter.kategori_nilai_id, "Kategori nilai wajib dipilih.")?;
    let jenis_penilaian_id = positive(filter.jenis_penilaian_id, "Jenis penilaian wajib dipilih.")?;
    let tingkat = fetch_class_level(db, kelas_id)
        .await
        .map_err(|message| (StatusCode::BAD_REQUEST, message))?;

    match validated_ujian_id(db, Some(ujian_id), mata_pelajaran_id, &tahun).await {
        Ok(Some(_)) => {}
        Ok(None) | Err(StatusCode::BAD_REQUEST) => {
            return Err((
                StatusCode::BAD_REQUEST,
                "Ujian tidak sesuai dengan tahun dan mata pelajaran terpilih.".into(),
            ));
        }
        Err(status) => return Err((status, "Gagal memvalidasi ujian.".into())),
    }

    let reference_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM penilaian_rapors
        WHERE tahun = ? AND semester = ? AND tingkat = ?
          AND kategori_nilai_id = ? AND jenis_penilaian_id = ?
        "#,
    )
    .bind(&tahun)
    .bind(semester.to_string())
    .bind(tingkat)
    .bind(kategori_nilai_id)
    .bind(jenis_penilaian_id)
    .fetch_one(db)
    .await
    .map_err(internal_error("memvalidasi referensi penilaian"))?;
    if reference_count != 1 {
        return Err((
            StatusCode::BAD_REQUEST,
            "Kombinasi kategori dan jenis penilaian tidak valid untuk tahun, semester, dan tingkat ini."
                .into(),
        ));
    }

    let source_rows =
        fetch_nilai_kelas_rows(db, kelas_id, mata_pelajaran_id, &tahun, Some(ujian_id)).await;
    let existing = sqlx::query_as::<_, ExistingScore>(
        r#"
        SELECT nis, MAX(nilai) AS nilai, COUNT(*) AS record_count
        FROM penilaians
        WHERE tahun = ? AND semester = ? AND kelas_id = ?
          AND mata_pelajaran_id = ? AND kategori_nilai_id = ? AND jenis_penilaian_id = ?
        GROUP BY nis
        "#,
    )
    .bind(&tahun)
    .bind(semester.to_string())
    .bind(kelas_id)
    .bind(mata_pelajaran_id)
    .bind(kategori_nilai_id)
    .bind(jenis_penilaian_id)
    .fetch_all(db)
    .await
    .map_err(internal_error("membaca nilai target"))?;
    let existing_map = existing
        .into_iter()
        .map(|row| (row.nis, (row.nilai, row.record_count)))
        .collect::<HashMap<_, _>>();

    let mut seen_nis = HashSet::new();
    let mut rows = Vec::with_capacity(source_rows.len());
    for source in source_rows {
        let nis_numeric = parse_nis(&source.nis);
        let duplicate_student = nis_numeric.is_some_and(|nis| !seen_nis.insert(nis));
        let target = nis_numeric.and_then(|nis| existing_map.get(&nis).copied());
        let (nilai_target, target_count) = target.unwrap_or((None, 0));
        let (nilai_sync, mut status, mut alasan) = score_for_sync(source.total_nilai);

        if nis_numeric.is_none() {
            status = SyncStatus::Skipped;
            alasan = Some("NIS tidak kompatibel dengan kolom numeric target.".into());
        } else if duplicate_student || target_count > 1 {
            status = SyncStatus::Conflict;
            alasan = Some(
                if duplicate_student {
                    "NIS terdaftar lebih dari sekali pada kelas dan tahun ini."
                } else {
                    "Lebih dari satu record target memiliki identitas akademik yang sama."
                }
                .into(),
            );
        } else if let Some(sync_value) = nilai_sync {
            status = match target_count {
                0 => SyncStatus::Insert,
                1 if nilai_target == Some(sync_value) => SyncStatus::Unchanged,
                1 => SyncStatus::Update,
                _ => status,
            };
        }

        rows.push(PreviewRow {
            nis: source.nis,
            nis_numeric,
            nama: source.nama,
            kelas: source.kelas,
            total_benar: source.total_benar,
            total_salah: source.total_salah,
            nilai_pg: source.nilai_pg,
            nilai_uraian: source.nilai_uraian,
            total_nilai: source.total_nilai,
            nilai_target,
            nilai_sync,
            status,
            alasan,
        });
    }

    let summary = summarize(&rows);
    Ok((
        rows,
        summary,
        ValidatedContext {
            tahun,
            semester,
            kelas_id,
            tingkat,
            mata_pelajaran_id,
            kategori_nilai_id,
            jenis_penilaian_id,
        },
    ))
}

fn score_for_sync(score: Option<f64>) -> (Option<i32>, SyncStatus, Option<String>) {
    let Some(score) = score else {
        return (
            None,
            SyncStatus::Skipped,
            Some("Nilai final ujian belum tersedia.".into()),
        );
    };
    if !score.is_finite() || !(0.0..=100.0).contains(&score) {
        return (
            None,
            SyncStatus::Skipped,
            Some("Nilai final berada di luar rentang 0–100.".into()),
        );
    }
    if score.fract().abs() > f64::EPSILON {
        return (
            None,
            SyncStatus::Skipped,
            Some("Nilai desimal tidak dapat disimpan tepat ke kolom nilai bertipe INT.".into()),
        );
    }
    (Some(score as i32), SyncStatus::Insert, None)
}

fn parse_nis(nis: &str) -> Option<u32> {
    let trimmed = nis.trim();
    (!trimmed.is_empty() && trimmed.bytes().all(|byte| byte.is_ascii_digit()))
        .then(|| trimmed.parse::<u32>().ok())
        .flatten()
        .filter(|value| *value <= 0x00ff_ffff)
}

fn summarize(rows: &[PreviewRow]) -> PreviewSummary {
    let mut summary = PreviewSummary::default();
    for row in rows {
        match row.status {
            SyncStatus::Insert => summary.insert += 1,
            SyncStatus::Update => summary.update += 1,
            SyncStatus::Unchanged => summary.unchanged += 1,
            SyncStatus::Skipped => summary.skipped += 1,
            SyncStatus::Conflict => summary.conflict += 1,
        }
    }
    summary.writable = summary.insert + summary.update;
    summary
}

async fn fetch_class_level(db: &MySqlPool, kelas_id: i64) -> Result<u8, String> {
    if kelas_id <= 0 {
        return Err("Kelas wajib dipilih.".into());
    }
    let level = sqlx::query_scalar::<_, String>("SELECT tingkat FROM kelas WHERE id = ? LIMIT 1")
        .bind(kelas_id)
        .fetch_optional(db)
        .await
        .map_err(|error| {
            eprintln!("ERROR fetch class level: {error:?}");
            "Gagal membaca tingkat kelas.".to_string()
        })?
        .ok_or_else(|| "Kelas tidak ditemukan.".to_string())?;
    level
        .trim()
        .parse::<u8>()
        .map_err(|_| "Tingkat kelas tidak valid.".to_string())
}

async fn fetch_references(
    db: &MySqlPool,
    tahun: &str,
    semester: u8,
    tingkat: u8,
) -> Result<Vec<ReferenceOption>, sqlx::Error> {
    sqlx::query_as::<_, ReferenceOption>(
        r#"
        SELECT DISTINCT
            CAST(pr.kategori_nilai_id AS SIGNED) AS kategori_nilai_id,
            kn.nama AS kategori_nilai,
            CAST(pr.jenis_penilaian_id AS SIGNED) AS jenis_penilaian_id,
            jp.nama AS jenis_penilaian
        FROM penilaian_rapors pr
        JOIN kategori_nilais kn ON kn.id = pr.kategori_nilai_id
        JOIN jenis_penilaians jp
          ON jp.id = pr.jenis_penilaian_id
         AND jp.kategori_nilai_id = pr.kategori_nilai_id
        WHERE pr.tahun = ? AND pr.semester = ? AND pr.tingkat = ?
        ORDER BY kn.nama, jp.nama
        "#,
    )
    .bind(tahun)
    .bind(semester.to_string())
    .bind(tingkat)
    .fetch_all(db)
    .await
}

fn render_preview(
    ctx: &PageContext,
    rows: Vec<PreviewRow>,
    summary: PreviewSummary,
    executed: bool,
    flash_message: Option<String>,
) -> Response {
    let mut tera_ctx = Context::new();
    tera_ctx.insert(
        "data",
        &PreviewData {
            rows,
            summary,
            executed,
        },
    );
    let html = ctx
        .tera
        .render("admin/_sinkron_preview.html", &tera_ctx)
        .unwrap_or_default();
    let mut headers = HeaderMap::new();
    if let Some(message) = flash_message {
        let payload = serde_json::json!({
            "flash": { "message": message, "type": "success" }
        });
        if let Ok(value) = HeaderValue::from_str(&payload.to_string()) {
            headers.insert("HX-Trigger", value);
        }
    }
    (headers, Html(html)).into_response()
}

fn render_preview_error(ctx: &PageContext, status: StatusCode, message: &str) -> Response {
    let mut tera_ctx = Context::new();
    tera_ctx.insert("message", message);
    let html = ctx
        .tera
        .render("admin/_sinkron_error.html", &tera_ctx)
        .unwrap_or_else(|_| message.to_string());
    (status, Html(html)).into_response()
}

fn positive(value: Option<i64>, message: &str) -> Result<i64, (StatusCode, String)> {
    value
        .filter(|value| *value > 0)
        .ok_or_else(|| (StatusCode::BAD_REQUEST, message.to_string()))
}

trait OptionBadRequest<T> {
    fn ok_or_bad_request(self, message: &str) -> Result<T, (StatusCode, String)>;
}

impl<T> OptionBadRequest<T> for Option<T> {
    fn ok_or_bad_request(self, message: &str) -> Result<T, (StatusCode, String)> {
        self.ok_or_else(|| (StatusCode::BAD_REQUEST, message.to_string()))
    }
}

fn internal_error(action: &'static str) -> impl FnOnce(sqlx::Error) -> (StatusCode, String) {
    move |error| {
        eprintln!("ERROR {action}: {error:?}");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Gagal {action}."),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Extension, Form, http::Uri};
    use serial_test::serial;

    use crate::{
        models::{auth_user::AccountType, role::Role},
        test_support::{SeedData, TestDb, seed_base_data},
    };

    #[test]
    fn zero_is_a_valid_final_score() {
        let (value, status, reason) = score_for_sync(Some(0.0));
        assert_eq!(value, Some(0));
        assert_eq!(status, SyncStatus::Insert);
        assert!(reason.is_none());
    }

    #[test]
    fn missing_and_decimal_scores_are_skipped_without_rounding() {
        assert_eq!(score_for_sync(None).1, SyncStatus::Skipped);
        let (value, status, reason) = score_for_sync(Some(79.5));
        assert_eq!(value, None);
        assert_eq!(status, SyncStatus::Skipped);
        assert!(reason.unwrap().contains("INT"));
    }

    #[test]
    fn nis_validation_preserves_display_but_resolves_numeric_target() {
        assert_eq!(parse_nis("00123"), Some(123));
        assert_eq!(parse_nis("12A3"), None);
        assert_eq!(parse_nis("16777216"), None);
    }

    #[test]
    fn summary_distinguishes_all_sync_states() {
        let statuses = [
            SyncStatus::Insert,
            SyncStatus::Update,
            SyncStatus::Unchanged,
            SyncStatus::Skipped,
            SyncStatus::Conflict,
        ];
        let rows = statuses
            .into_iter()
            .map(|status| PreviewRow {
                nis: "1".into(),
                nis_numeric: Some(1),
                nama: None,
                kelas: None,
                total_benar: None,
                total_salah: None,
                nilai_pg: None,
                nilai_uraian: None,
                total_nilai: None,
                nilai_target: None,
                nilai_sync: None,
                status,
                alasan: None,
            })
            .collect::<Vec<_>>();
        let summary = summarize(&rows);
        assert_eq!(summary.insert, 1);
        assert_eq!(summary.update, 1);
        assert_eq!(summary.unchanged, 1);
        assert_eq!(summary.skipped, 1);
        assert_eq!(summary.conflict, 1);
        assert_eq!(summary.writable, 2);
    }

    async fn seed_sync_context(db: &MySqlPool, seed: &SeedData, score: f64) -> SinkronFilter {
        sqlx::query("INSERT INTO mata_pelajarans (nama, kelompok) VALUES ('Informatika', 'B')")
            .execute(db)
            .await
            .unwrap();
        let mapel_id: i64 = sqlx::query_scalar("SELECT CAST(LAST_INSERT_ID() AS SIGNED)")
            .fetch_one(db)
            .await
            .unwrap();
        sqlx::query(
            r#"INSERT INTO ujians
               (mata_pelajaran_id, tahun, title, tanggal, waktu_menit)
               VALUES (?, ?, 'Ujian Sinkron', CURRENT_DATE, 60)"#,
        )
        .bind(mapel_id)
        .bind(&seed.tahun)
        .execute(db)
        .await
        .unwrap();
        let ujian_id: i64 = sqlx::query_scalar("SELECT CAST(LAST_INSERT_ID() AS SIGNED)")
            .fetch_one(db)
            .await
            .unwrap();
        sqlx::query("INSERT INTO kategori_nilais (nama) VALUES ('Sumatif')")
            .execute(db)
            .await
            .unwrap();
        let category_id: i64 = sqlx::query_scalar("SELECT CAST(LAST_INSERT_ID() AS SIGNED)")
            .fetch_one(db)
            .await
            .unwrap();
        sqlx::query("INSERT INTO jenis_penilaians (nama, kategori_nilai_id) VALUES ('STS', ?)")
            .bind(category_id)
            .execute(db)
            .await
            .unwrap();
        let type_id: i64 = sqlx::query_scalar("SELECT CAST(LAST_INSERT_ID() AS SIGNED)")
            .fetch_one(db)
            .await
            .unwrap();
        sqlx::query(
            r#"INSERT INTO penilaian_rapors
               (tingkat, tahun, semester, kategori_nilai_id, jenis_penilaian_id)
               VALUES (7, ?, '1', ?, ?)"#,
        )
        .bind(&seed.tahun)
        .bind(category_id)
        .bind(type_id)
        .execute(db)
        .await
        .unwrap();
        sqlx::query(
            r#"INSERT INTO hasil_nilais
               (nis, ujian_id, mata_pelajaran_id, tahun, total_benar, total_salah,
                total_pg, total_uraian, total_nilai)
               VALUES (?, ?, ?, ?, 8, 2, ?, 0, ?)"#,
        )
        .bind(&seed.nis)
        .bind(ujian_id)
        .bind(mapel_id)
        .bind(&seed.tahun)
        .bind(score)
        .bind(score)
        .execute(db)
        .await
        .unwrap();

        SinkronFilter {
            tahun: Some(seed.tahun.clone()),
            semester: Some(1),
            kelas_id: Some(seed.kelas_id),
            mata_pelajaran_id: Some(mapel_id),
            ujian_id: Some(ujian_id),
            kategori_nilai_id: Some(category_id),
            jenis_penilaian_id: Some(type_id),
        }
    }

    fn page_context(seed: &SeedData) -> PageContext {
        PageContext {
            user: crate::models::auth_user::AuthUser {
                id: seed.admin_id,
                account_type: AccountType::Web,
                nis: None,
                roles: vec![Role::Admin],
                name: "Admin".into(),
                foto: None,
            },
            tera: crate::test_support::build_test_tera(),
            uri: Uri::from_static("/sinkron-penilaian"),
        }
    }

    #[tokio::test]
    #[serial]
    async fn preview_uses_exact_exam_score_and_validates_reference_pair() {
        let Some(test_db) = TestDb::try_new().await else {
            return;
        };
        let seed = seed_base_data(&test_db.pool).await;
        let mut filter = seed_sync_context(&test_db.pool, &seed, 83.0).await;

        let (rows, summary, validated) = build_preview(&test_db.pool, &filter).await.unwrap();
        assert_eq!(validated.tingkat, 7);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].nis, seed.nis);
        assert_eq!(rows[0].total_nilai, Some(83.0));
        assert_eq!(rows[0].nilai_sync, Some(83));
        assert_eq!(rows[0].status, SyncStatus::Insert);
        assert_eq!(summary.insert, 1);

        filter.jenis_penilaian_id = Some(999);
        let error = build_preview(&test_db.pool, &filter).await.unwrap_err();
        assert_eq!(error.0, StatusCode::BAD_REQUEST);
        test_db.teardown().await;
    }

    #[tokio::test]
    #[serial]
    async fn execute_inserts_updates_and_then_becomes_unchanged_without_duplicates() {
        let Some(test_db) = TestDb::try_new().await else {
            return;
        };
        let seed = seed_base_data(&test_db.pool).await;
        let filter = seed_sync_context(&test_db.pool, &seed, 0.0).await;

        let response = sinkron_execute(
            page_context(&seed),
            Extension(test_db.pool.clone()),
            Form(filter.clone()),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let stored: (i32, u32, i64) =
            sqlx::query_as("SELECT nilai, nis, COUNT(*) OVER () FROM penilaians LIMIT 1")
                .fetch_one(&test_db.pool)
                .await
                .unwrap();
        assert_eq!(stored, (0, seed.nis.parse().unwrap(), 1));

        sqlx::query("UPDATE hasil_nilais SET total_nilai = 91 WHERE ujian_id = ?")
            .bind(filter.ujian_id)
            .execute(&test_db.pool)
            .await
            .unwrap();
        let response = sinkron_execute(
            page_context(&seed),
            Extension(test_db.pool.clone()),
            Form(filter.clone()),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let value: i32 = sqlx::query_scalar("SELECT nilai FROM penilaians")
            .fetch_one(&test_db.pool)
            .await
            .unwrap();
        assert_eq!(value, 91);

        let (_, summary, _) = build_preview(&test_db.pool, &filter).await.unwrap();
        assert_eq!(summary.unchanged, 1);
        assert_eq!(summary.insert, 0);
        assert_eq!(summary.update, 0);
        test_db.teardown().await;
    }

    #[tokio::test]
    #[serial]
    async fn semester_and_category_are_isolated_and_ujian_id_is_not_stored() {
        let Some(test_db) = TestDb::try_new().await else {
            return;
        };
        let seed = seed_base_data(&test_db.pool).await;
        let filter = seed_sync_context(&test_db.pool, &seed, 77.0).await;
        sinkron_execute(
            page_context(&seed),
            Extension(test_db.pool.clone()),
            Form(filter.clone()),
        )
        .await;

        sqlx::query("INSERT INTO kategori_nilais (nama) VALUES ('Formatif')")
            .execute(&test_db.pool)
            .await
            .unwrap();
        let category_2: i64 = sqlx::query_scalar("SELECT CAST(LAST_INSERT_ID() AS SIGNED)")
            .fetch_one(&test_db.pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO jenis_penilaians (nama, kategori_nilai_id) VALUES ('TP 3', ?)")
            .bind(category_2)
            .execute(&test_db.pool)
            .await
            .unwrap();
        let type_2: i64 = sqlx::query_scalar("SELECT CAST(LAST_INSERT_ID() AS SIGNED)")
            .fetch_one(&test_db.pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO penilaian_rapors (tingkat,tahun,semester,kategori_nilai_id,jenis_penilaian_id) VALUES (7,?,'2',?,?)",
        )
        .bind(&seed.tahun)
        .bind(category_2)
        .bind(type_2)
        .execute(&test_db.pool)
        .await
        .unwrap();
        let second_filter = SinkronFilter {
            semester: Some(2),
            kategori_nilai_id: Some(category_2),
            jenis_penilaian_id: Some(type_2),
            ..filter
        };
        sinkron_execute(
            page_context(&seed),
            Extension(test_db.pool.clone()),
            Form(second_filter),
        )
        .await;

        let rows: Vec<(String, i64, i64, i32)> = sqlx::query_as(
            "SELECT semester, CAST(kategori_nilai_id AS SIGNED), CAST(jenis_penilaian_id AS SIGNED), nilai FROM penilaians ORDER BY semester",
        )
        .fetch_all(&test_db.pool)
        .await
        .unwrap();
        assert_eq!(rows.len(), 2);
        assert_ne!(
            (rows[0].0.clone(), rows[0].1, rows[0].2),
            (rows[1].0.clone(), rows[1].1, rows[1].2)
        );
        let ujian_column: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM information_schema.columns WHERE table_schema = DATABASE() AND table_name = 'penilaians' AND column_name = 'ujian_id'",
        )
        .fetch_one(&test_db.pool)
        .await
        .unwrap();
        assert_eq!(ujian_column, 0);
        test_db.teardown().await;
    }

    #[tokio::test]
    #[serial]
    async fn ambiguous_target_is_reported_as_conflict() {
        let Some(test_db) = TestDb::try_new().await else {
            return;
        };
        let seed = seed_base_data(&test_db.pool).await;
        let filter = seed_sync_context(&test_db.pool, &seed, 75.0).await;
        sqlx::query("ALTER TABLE penilaians DROP INDEX penilaians_unique")
            .execute(&test_db.pool)
            .await
            .unwrap();
        for _ in 0..2 {
            sqlx::query(
                "INSERT INTO penilaians (nis,tahun,semester,kelas_id,mata_pelajaran_id,kategori_nilai_id,jenis_penilaian_id,nilai) VALUES (?,?,?,?,?,?,?,70)",
            )
            .bind(seed.nis.parse::<u32>().unwrap())
            .bind(&seed.tahun)
            .bind("1")
            .bind(filter.kelas_id)
            .bind(filter.mata_pelajaran_id)
            .bind(filter.kategori_nilai_id)
            .bind(filter.jenis_penilaian_id)
            .execute(&test_db.pool)
            .await
            .unwrap();
        }
        let (rows, summary, _) = build_preview(&test_db.pool, &filter).await.unwrap();
        assert_eq!(rows[0].status, SyncStatus::Conflict);
        assert_eq!(summary.conflict, 1);
        assert_eq!(summary.writable, 0);
        test_db.teardown().await;
    }

    #[tokio::test]
    #[serial]
    async fn failed_batch_rolls_back_earlier_writes() {
        let Some(test_db) = TestDb::try_new().await else {
            return;
        };
        let seed = seed_base_data(&test_db.pool).await;
        let filter = seed_sync_context(&test_db.pool, &seed, 80.0).await;
        sqlx::query("INSERT INTO user_siswa (name,password,nis) VALUES ('ZZZ Siswa','x',22222)")
            .execute(&test_db.pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO siswas (nis,tahun,kelas_id,tingkat) VALUES (22222,?,?,7)")
            .bind(&seed.tahun)
            .bind(seed.kelas_id)
            .execute(&test_db.pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO hasil_nilais (nis,ujian_id,mata_pelajaran_id,tahun,total_nilai) VALUES ('22222',?,?,?,82)",
        )
        .bind(filter.ujian_id)
        .bind(filter.mata_pelajaran_id)
        .bind(&seed.tahun)
        .execute(&test_db.pool)
        .await
        .unwrap();
        sqlx::query(
            "CREATE TRIGGER fail_second_score BEFORE INSERT ON penilaians FOR EACH ROW BEGIN IF NEW.nis = 22222 THEN SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT = 'forced failure'; END IF; END",
        )
        .execute(&test_db.pool)
        .await
        .unwrap();

        let response = sinkron_execute(
            page_context(&seed),
            Extension(test_db.pool.clone()),
            Form(filter),
        )
        .await;
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM penilaians")
            .fetch_one(&test_db.pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
        test_db.teardown().await;
    }

    #[tokio::test]
    #[serial]
    async fn concurrent_sync_keeps_single_composite_identity() {
        let Some(test_db) = TestDb::try_new().await else {
            return;
        };
        let seed = seed_base_data(&test_db.pool).await;
        let filter = seed_sync_context(&test_db.pool, &seed, 88.0).await;
        let left = sinkron_execute(
            page_context(&seed),
            Extension(test_db.pool.clone()),
            Form(filter.clone()),
        );
        let right = sinkron_execute(
            page_context(&seed),
            Extension(test_db.pool.clone()),
            Form(filter),
        );
        let (left, right) = tokio::join!(left, right);
        assert_eq!(left.status(), StatusCode::OK);
        assert_eq!(right.status(), StatusCode::OK);
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM penilaians")
            .fetch_one(&test_db.pool)
            .await
            .unwrap();
        assert_eq!(count, 1);
        test_db.teardown().await;
    }
}
