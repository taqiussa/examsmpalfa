use std::{
    collections::{HashMap, HashSet},
    fmt::Display,
    str::FromStr,
};

use axum::{
    Form,
    extract::Query,
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Response},
};
use serde::{Deserialize, Deserializer, Serialize, de};
use sqlx::{MySqlConnection, MySqlPool};
use tera::Context;

use crate::{
    controllers::guru::ujian::{
        MapelOption, NilaiKelasRow, fetch_mapel_options, nilai::fetch_nilai_kelas_rows,
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
    #[serde(default, deserialize_with = "deserialize_optional_number")]
    pub semester: Option<u8>,
    #[serde(default, deserialize_with = "deserialize_optional_number")]
    pub kelas_id: Option<i64>,
    #[serde(default, deserialize_with = "deserialize_optional_number")]
    pub mata_pelajaran_id: Option<i64>,
    #[serde(default, deserialize_with = "deserialize_optional_number")]
    pub ujian_id: Option<i64>,
    #[serde(default, deserialize_with = "deserialize_optional_number")]
    pub kategori_nilai_id: Option<i64>,
    #[serde(default, deserialize_with = "deserialize_optional_number")]
    pub jenis_penilaian_id: Option<i64>,
    /// Monotonic client-side request token used only to prevent stale HTMX
    /// reference responses from replacing newer form state.
    #[serde(default, deserialize_with = "deserialize_optional_number")]
    pub reference_token: Option<u64>,
}

/// Deserializes HTML form/query number fields where an unselected `<select>`
/// is submitted as an empty string. Invalid non-empty values remain errors so
/// Axum can safely reject malformed requests before the handler runs.
fn deserialize_optional_number<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: FromStr,
    T::Err: Display,
{
    let value = Option::<String>::deserialize(deserializer)?;

    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.parse::<T>().map_err(de::Error::custom))
        .transpose()
}

#[derive(Serialize)]
struct SinkronPageData {
    tahun: String,
    semester: u8,
    list_kelas: Vec<ListKelas>,
    list_mapel: Vec<MapelOption>,
    list_ujian: Vec<SinkronUjianOption>,
    kelas_id: i64,
    mata_pelajaran_id: i64,
    ujian_id: i64,
    kategori_nilai_id: i64,
    jenis_penilaian_id: i64,
    require_kelas: bool,
    categories: Vec<ReferenceCategory>,
    types: Vec<ReferenceType>,
    tingkat: Option<u8>,
    reference_error: Option<String>,
    request_token: u64,
}

#[derive(Clone, Serialize, sqlx::FromRow)]
struct SinkronUjianOption {
    id: i64,
    title: String,
    mata_pelajaran: String,
    tahun: String,
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
    error: Option<String>,
    request_token: u64,
}

#[derive(Clone, Serialize)]
struct ReferenceCategory {
    id: i64,
    nama: String,
}

#[derive(Clone, Serialize)]
struct ReferenceType {
    id: i64,
    nama: String,
}

#[derive(Serialize)]
struct UjianOptionsData {
    list_ujian: Vec<SinkronUjianOption>,
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

#[derive(sqlx::FromRow)]
struct SourceUjian {
    mata_pelajaran_id: i64,
}

#[derive(sqlx::FromRow)]
struct RaporScore {
    nis: u32,
    total_nilai: i64,
}

pub async fn sinkron_penilaian_page(
    ctx: PageContext,
    Query(filter): Query<SinkronFilter>,
    axum::Extension(db): axum::Extension<MySqlPool>,
) -> Html<String> {
    let tahun = filter.tahun.clone().unwrap_or_else(data_tahun);
    let semester = filter.semester.unwrap_or_else(|| data_semester() as u8);
    let mata_pelajaran_id = filter.mata_pelajaran_id.unwrap_or(0);
    let list_ujian = fetch_sinkron_ujian_options(&db).await;
    let ujian_id = filter
        .ujian_id
        .filter(|id| list_ujian.iter().any(|ujian| ujian.id == *id))
        .unwrap_or(0);

    let references = fetch_reference_data(&db, &filter, &tahun, semester).await;
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
        categories: references.categories,
        types: references.types,
        tingkat: references.tingkat,
        reference_error: references.error,
        request_token: 0,
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
    let options = fetch_sinkron_ujian_options(&db).await;
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
    let tahun = filter.tahun.clone().unwrap_or_else(data_tahun);
    let semester = filter.semester.unwrap_or_else(|| data_semester() as u8);
    let references = fetch_reference_data(&db, &filter, &tahun, semester).await;

    let mut tera_ctx = Context::new();
    tera_ctx.insert("data", &references);
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

    // Laravel checks the grade lock before it writes. Do the same check inside
    // this transaction so a lock enabled after preview cannot allow a partial
    // sync to proceed.
    let grade_lock = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT is_active
        FROM kunci_nilais
        WHERE tahun = ? AND semester = ? AND tingkat = ?
        LIMIT 1 FOR UPDATE
        "#,
    )
    .bind(&validated.tahun)
    .bind(validated.semester.to_string())
    .bind(validated.tingkat)
    .fetch_optional(&mut *tx)
    .await;

    match grade_lock {
        Ok(Some(false)) => {
            let _ = tx.rollback().await;
            return render_preview_error(
                &ctx,
                StatusCode::CONFLICT,
                "Nilai telah dikunci. Hubungi Kurikulum untuk membuka kunci nilai.",
            );
        }
        Ok(_) => {}
        Err(error) => {
            eprintln!("ERROR check sinkron grade lock: {error:?}");
            let _ = tx.rollback().await;
            return render_preview_error(
                &ctx,
                StatusCode::INTERNAL_SERVER_ERROR,
                "Gagal memeriksa kunci nilai.",
            );
        }
    }

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

    let rapor_type_ids = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT CAST(id AS SIGNED)
        FROM penilaian_rapors
        WHERE tahun = ? AND semester = ? AND tingkat = ?
        FOR UPDATE
        "#,
    )
    .bind(&validated.tahun)
    .bind(validated.semester.to_string())
    .bind(validated.tingkat)
    .fetch_all(&mut *tx)
    .await;
    let rapor_type_count = match rapor_type_ids {
        Ok(ids) if !ids.is_empty() => ids.len() as i64,
        Ok(_) => {
            let _ = tx.rollback().await;
            return render_preview_error(
                &ctx,
                StatusCode::CONFLICT,
                "Penilaian rapor belum diatur untuk tahun, semester, dan tingkat ini.",
            );
        }
        Err(error) => {
            eprintln!("ERROR count sinkron rapor types: {error:?}");
            let _ = tx.rollback().await;
            return render_preview_error(
                &ctx,
                StatusCode::INTERNAL_SERVER_ERROR,
                "Gagal membaca konfigurasi penilaian rapor.",
            );
        }
    };

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

    if let Err(error) =
        recalculate_nilai_rapors(&mut tx, &validated, rapor_type_count, ctx.user.id).await
    {
        eprintln!("ERROR recalculate sinkron nilai rapor: {error:?}");
        let _ = tx.rollback().await;
        return render_preview_error(
            &ctx,
            StatusCode::INTERNAL_SERVER_ERROR,
            "Sinkronisasi gagal menghitung nilai rapor; seluruh perubahan batch dibatalkan.",
        );
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

/// Reproduces `InputNilaiSimpanController`: the denominator is every report
/// assessment configured for the class level, while the numerator is all of a
/// student's assessment scores for this period and subject. Laravel deliberately
/// does not filter that relation by `kelas_id`, so neither do we here.
async fn recalculate_nilai_rapors(
    connection: &mut MySqlConnection,
    context: &ValidatedContext,
    rapor_type_count: i64,
    user_id: u64,
) -> Result<(), sqlx::Error> {
    let scores = sqlx::query_as::<_, RaporScore>(
        r#"
        SELECT s.nis, CAST(COALESCE(SUM(p.nilai), 0) AS SIGNED) AS total_nilai
        FROM siswas s
        LEFT JOIN penilaians p
          ON p.nis = s.nis
         AND p.tahun = ?
         AND p.semester = ?
         AND p.mata_pelajaran_id = ?
        WHERE s.tahun = ? AND s.kelas_id = ?
        GROUP BY s.nis
        FOR UPDATE
        "#,
    )
    .bind(&context.tahun)
    .bind(context.semester.to_string())
    .bind(context.mata_pelajaran_id)
    .bind(&context.tahun)
    .bind(context.kelas_id)
    .fetch_all(&mut *connection)
    .await?;

    for score in scores {
        // `total_nilai` and the count originate from integer Laravel columns;
        // integer division provides the same floor behavior for valid scores.
        let nilai = score.total_nilai.div_euclid(rapor_type_count) as i32;
        sqlx::query(
            r#"
            INSERT INTO nilai_rapors
                (tahun, semester, kelas_id, mata_pelajaran_id, nis, nilai, user_id, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, NOW(), NOW())
            ON DUPLICATE KEY UPDATE
                nilai = VALUES(nilai),
                user_id = VALUES(user_id),
                updated_at = NOW()
            "#,
        )
        .bind(&context.tahun)
        .bind(context.semester.to_string())
        .bind(context.kelas_id)
        .bind(context.mata_pelajaran_id)
        .bind(score.nis)
        .bind(nilai)
        .bind(user_id)
        .execute(&mut *connection)
        .await?;
    }

    Ok(())
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

    let source_ujian = validate_sinkron_ujian(db, ujian_id).await?;
    if source_ujian.mata_pelajaran_id != mata_pelajaran_id {
        return Err((
            StatusCode::BAD_REQUEST,
            "Ujian yang dipilih berasal dari mata pelajaran lain. Pilih mata pelajaran tujuan yang sama dengan ujian.".into(),
        ));
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

    let source_rows = fetch_sinkron_nilai_kelas_rows(
        db,
        kelas_id,
        &tahun,
        ujian_id,
        source_ujian.mata_pelajaran_id,
    )
    .await;
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
    // `kelas.tingkat` is VARCHAR in the Laravel schema. Casting it to CHAR also
    // keeps this decode compatible with installations that use a numeric column.
    let level = sqlx::query_scalar::<_, String>(
        "SELECT CAST(tingkat AS CHAR) FROM kelas WHERE id = ? LIMIT 1",
    )
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

async fn fetch_sinkron_ujian_options(db: &MySqlPool) -> Vec<SinkronUjianOption> {
    sqlx::query_as::<_, SinkronUjianOption>(
        r#"
        SELECT u.id, u.title, COALESCE(mp.nama, '-') AS mata_pelajaran, u.tahun
        FROM ujians u
        LEFT JOIN mata_pelajarans mp ON mp.id = u.mata_pelajaran_id
        ORDER BY u.tanggal DESC, u.id DESC
        "#,
    )
    .fetch_all(db)
    .await
    .unwrap_or_else(|error| {
        eprintln!("ERROR fetch sinkron ujian options: {error:?}");
        Vec::new()
    })
}

async fn validate_sinkron_ujian(
    db: &MySqlPool,
    ujian_id: i64,
) -> Result<SourceUjian, (StatusCode, String)> {
    sqlx::query_as::<_, SourceUjian>("SELECT mata_pelajaran_id FROM ujians WHERE id = ? LIMIT 1")
        .bind(ujian_id)
        .fetch_optional(db)
        .await
        .map_err(internal_error("memvalidasi ujian"))?
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                "Ujian yang dipilih tidak ditemukan.".into(),
            )
        })
}

async fn fetch_sinkron_nilai_kelas_rows(
    db: &MySqlPool,
    kelas_id: i64,
    tahun_tujuan: &str,
    ujian_id: i64,
    mata_pelajaran_id: i64,
) -> Vec<NilaiKelasRow> {
    // Daftar siswa tetap mengikuti konteks penilaian tujuan, sedangkan hasil
    // selalu diikat ke identitas ujian sumber (bukan tahun tujuan).
    let mut rows = fetch_nilai_kelas_rows(db, kelas_id, 0, tahun_tujuan, None).await;
    for row in &mut rows {
        row.total_benar = None;
        row.total_salah = None;
        row.nilai_pg = None;
        row.nilai_uraian = None;
        row.total_nilai = None;
    }

    let hasil = sqlx::query_as::<_, (String, f64, f64, f64, f64, f64)>(
        r#"
        SELECT TRIM(nis) AS nis,
            CAST(COALESCE(total_benar, 0) AS DOUBLE),
            CAST(COALESCE(total_salah, 0) AS DOUBLE),
            CAST(COALESCE(total_pg, 0) AS DOUBLE),
            CAST(COALESCE(total_uraian, 0) AS DOUBLE),
            CAST(COALESCE(total_nilai, 0) AS DOUBLE)
        FROM hasil_nilais
        WHERE ujian_id = ? AND mata_pelajaran_id = ?
        "#,
    )
    .bind(ujian_id)
    .bind(mata_pelajaran_id)
    .fetch_all(db)
    .await;

    let Ok(hasil) = hasil else {
        if let Err(error) = hasil {
            eprintln!("ERROR fetch sinkron hasil nilai: {error:?}");
        }
        return rows;
    };
    let nilai = hasil
        .into_iter()
        .map(|(nis, benar, salah, pg, uraian, total)| (nis, (benar, salah, pg, uraian, total)))
        .collect::<HashMap<_, _>>();
    for row in &mut rows {
        if let Some((benar, salah, pg, uraian, total)) = nilai.get(&row.nis) {
            row.total_benar = Some(*benar);
            row.total_salah = Some(*salah);
            row.nilai_pg = Some(*pg);
            row.nilai_uraian = Some(*uraian);
            row.total_nilai = Some(*total);
        }
    }
    rows
}

async fn fetch_reference_data(
    db: &MySqlPool,
    filter: &SinkronFilter,
    tahun: &str,
    semester: u8,
) -> ReferenceData {
    let request_token = filter.reference_token.unwrap_or(0);
    let Some(kelas_id) = filter.kelas_id.filter(|id| *id > 0) else {
        return ReferenceData {
            categories: Vec::new(),
            types: Vec::new(),
            kategori_nilai_id: 0,
            jenis_penilaian_id: 0,
            tingkat: None,
            error: None,
            request_token,
        };
    };
    let tingkat = match fetch_class_level(db, kelas_id).await {
        Ok(tingkat) => Some(tingkat),
        Err(message) => {
            return ReferenceData {
                categories: Vec::new(),
                types: Vec::new(),
                kategori_nilai_id: 0,
                jenis_penilaian_id: 0,
                tingkat: None,
                error: Some(message),
                request_token,
            };
        }
    };
    let references = match fetch_references(db, tahun, semester, tingkat.unwrap()).await {
        Ok(references) => references,
        Err(error) => {
            eprintln!("ERROR fetch sinkron references: {error:?}");
            return ReferenceData {
                categories: Vec::new(),
                types: Vec::new(),
                kategori_nilai_id: 0,
                jenis_penilaian_id: 0,
                tingkat,
                error: Some("Gagal memuat referensi penilaian. Coba lagi.".into()),
                request_token,
            };
        }
    };
    let requested_category = filter.kategori_nilai_id.unwrap_or(0);
    let kategori_nilai_id = references
        .iter()
        .any(|item| item.kategori_nilai_id == requested_category)
        .then_some(requested_category)
        .unwrap_or(0);
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
    ReferenceData {
        categories,
        types,
        kategori_nilai_id,
        jenis_penilaian_id,
        tingkat,
        error: None,
        request_token,
    }
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

    #[test]
    fn optional_numeric_query_fields_accept_empty_values() {
        // This reproduces the reference-options request sent while a class is
        // selected before a subject and exam have been selected.
        let filter: SinkronFilter = serde_urlencoded::from_str(
            "tahun=2026+%2F+2027&semester=1&kelas_id=2&mata_pelajaran_id=&ujian_id=&reference_token=2",
        )
        .unwrap();

        assert_eq!(filter.tahun.as_deref(), Some("2026 / 2027"));
        assert_eq!(filter.semester, Some(1));
        assert_eq!(filter.kelas_id, Some(2));
        assert_eq!(filter.mata_pelajaran_id, None);
        assert_eq!(filter.ujian_id, None);
        assert_eq!(filter.reference_token, Some(2));
    }

    #[test]
    fn optional_numeric_query_fields_reject_invalid_values() {
        assert!(serde_urlencoded::from_str::<SinkronFilter>("kelas_id=abc").is_err());
        assert!(serde_urlencoded::from_str::<SinkronFilter>("semester=xyz").is_err());
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
            reference_token: None,
        }
    }

    async fn add_rapor_score(
        db: &MySqlPool,
        seed: &SeedData,
        filter: &SinkronFilter,
        nilai: i32,
        suffix: &str,
    ) {
        let category_id = sqlx::query("INSERT INTO kategori_nilais (nama) VALUES (?)")
            .bind(format!("Kategori {suffix}"))
            .execute(db)
            .await
            .unwrap()
            .last_insert_id() as i64;
        let type_id =
            sqlx::query("INSERT INTO jenis_penilaians (nama, kategori_nilai_id) VALUES (?, ?)")
                .bind(format!("Jenis {suffix}"))
                .bind(category_id)
                .execute(db)
                .await
                .unwrap()
                .last_insert_id() as i64;
        sqlx::query(
            "INSERT INTO penilaian_rapors (tingkat, tahun, semester, kategori_nilai_id, jenis_penilaian_id) VALUES (7, ?, '1', ?, ?)",
        )
        .bind(&seed.tahun)
        .bind(category_id)
        .bind(type_id)
        .execute(db)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO penilaians (nis, tahun, semester, kelas_id, mata_pelajaran_id, kategori_nilai_id, jenis_penilaian_id, nilai) VALUES (?, ?, '1', ?, ?, ?, ?, ?)",
        )
        .bind(seed.nis.parse::<u32>().unwrap())
        .bind(&seed.tahun)
        .bind(filter.kelas_id)
        .bind(filter.mata_pelajaran_id)
        .bind(category_id)
        .bind(type_id)
        .bind(nilai)
        .execute(db)
        .await
        .unwrap();
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
    async fn sinkron_exam_options_include_inactive_old_and_other_subject_exams() {
        let Some(test_db) = TestDb::try_new().await else {
            return;
        };
        let seed = seed_base_data(&test_db.pool).await;
        let filter = seed_sync_context(&test_db.pool, &seed, 83.0).await;
        let other_mapel =
            sqlx::query("INSERT INTO mata_pelajarans (nama, kelompok) VALUES ('Sejarah', 'A')")
                .execute(&test_db.pool)
                .await
                .unwrap()
                .last_insert_id() as i64;
        let old_id = sqlx::query(
            "INSERT INTO ujians (mata_pelajaran_id, tahun, title, tanggal, waktu_menit, is_active) VALUES (?, '2020/2021', 'Ujian Lama Nonaktif', '2020-01-01', 60, 0)",
        )
        .bind(other_mapel)
        .execute(&test_db.pool)
        .await
        .unwrap()
        .last_insert_id() as i64;

        let options = fetch_sinkron_ujian_options(&test_db.pool).await;
        assert!(
            options
                .iter()
                .any(|option| option.id == filter.ujian_id.unwrap())
        );
        assert!(options.iter().any(|option| {
            option.id == old_id
                && option.title == "Ujian Lama Nonaktif"
                && option.mata_pelajaran == "Sejarah"
                && option.tahun == "2020/2021"
        }));
        test_db.teardown().await;
    }

    #[tokio::test]
    #[serial]
    async fn preview_reads_old_exam_by_exam_identity_and_rejects_subject_mismatch() {
        let Some(test_db) = TestDb::try_new().await else {
            return;
        };
        let seed = seed_base_data(&test_db.pool).await;
        let filter = seed_sync_context(&test_db.pool, &seed, 83.0).await;
        sqlx::query("UPDATE ujians SET tahun = '2020/2021' WHERE id = ?")
            .bind(filter.ujian_id)
            .execute(&test_db.pool)
            .await
            .unwrap();
        sqlx::query("UPDATE hasil_nilais SET tahun = '2020/2021' WHERE ujian_id = ?")
            .bind(filter.ujian_id)
            .execute(&test_db.pool)
            .await
            .unwrap();
        let (rows, _, _) = build_preview(&test_db.pool, &filter).await.unwrap();
        assert_eq!(rows[0].total_nilai, Some(83.0));

        let other_mapel =
            sqlx::query("INSERT INTO mata_pelajarans (nama, kelompok) VALUES ('IPS', 'A')")
                .execute(&test_db.pool)
                .await
                .unwrap()
                .last_insert_id() as i64;
        sqlx::query("UPDATE ujians SET mata_pelajaran_id = ? WHERE id = ?")
            .bind(other_mapel)
            .bind(filter.ujian_id)
            .execute(&test_db.pool)
            .await
            .unwrap();
        let error = build_preview(&test_db.pool, &filter).await.unwrap_err();
        assert_eq!(error.0, StatusCode::BAD_REQUEST);
        assert!(error.1.contains("mata pelajaran lain"));
        test_db.teardown().await;
    }

    #[tokio::test]
    #[serial]
    async fn reference_data_keeps_only_valid_category_type_pairs() {
        let Some(test_db) = TestDb::try_new().await else {
            return;
        };
        let seed = seed_base_data(&test_db.pool).await;
        let mut filter = seed_sync_context(&test_db.pool, &seed, 83.0).await;
        let data = fetch_reference_data(&test_db.pool, &filter, &seed.tahun, 1).await;
        assert_eq!(data.categories.len(), 1);
        assert_eq!(data.types.len(), 1);
        assert_eq!(data.jenis_penilaian_id, filter.jenis_penilaian_id.unwrap());

        filter.jenis_penilaian_id = Some(999);
        let data = fetch_reference_data(&test_db.pool, &filter, &seed.tahun, 1).await;
        assert_eq!(data.kategori_nilai_id, filter.kategori_nilai_id.unwrap());
        assert_eq!(data.jenis_penilaian_id, 0);
        assert_eq!(data.types.len(), 1);
        test_db.teardown().await;
    }

    #[tokio::test]
    #[serial]
    async fn class_level_is_read_from_the_selected_class_and_invalid_values_are_rejected() {
        let Some(test_db) = TestDb::try_new().await else {
            return;
        };
        let seed = seed_base_data(&test_db.pool).await;
        sqlx::query("INSERT INTO kelas (nama, tingkat) VALUES ('8A', '8'), ('9A', '9')")
            .execute(&test_db.pool)
            .await
            .unwrap();
        let kelas_8: i64 =
            sqlx::query_scalar("SELECT CAST(id AS SIGNED) FROM kelas WHERE nama = '8A'")
                .fetch_one(&test_db.pool)
                .await
                .unwrap();
        let kelas_9: i64 =
            sqlx::query_scalar("SELECT CAST(id AS SIGNED) FROM kelas WHERE nama = '9A'")
                .fetch_one(&test_db.pool)
                .await
                .unwrap();

        assert_eq!(fetch_class_level(&test_db.pool, seed.kelas_id).await, Ok(7));
        assert_eq!(fetch_class_level(&test_db.pool, kelas_8).await, Ok(8));
        assert_eq!(fetch_class_level(&test_db.pool, kelas_9).await, Ok(9));

        sqlx::query("UPDATE kelas SET tingkat = 'tidak-valid' WHERE id = ?")
            .bind(kelas_9)
            .execute(&test_db.pool)
            .await
            .unwrap();
        assert!(
            fetch_class_level(&test_db.pool, kelas_9)
                .await
                .unwrap_err()
                .contains("tidak valid")
        );
        test_db.teardown().await;
    }

    #[tokio::test]
    #[serial]
    async fn changing_class_resets_references_when_the_new_level_has_no_reference() {
        let Some(test_db) = TestDb::try_new().await else {
            return;
        };
        let seed = seed_base_data(&test_db.pool).await;
        let mut filter = seed_sync_context(&test_db.pool, &seed, 83.0).await;
        sqlx::query("INSERT INTO kelas (nama, tingkat) VALUES ('8A', '8')")
            .execute(&test_db.pool)
            .await
            .unwrap();
        let kelas_8: i64 =
            sqlx::query_scalar("SELECT CAST(id AS SIGNED) FROM kelas WHERE nama = '8A'")
                .fetch_one(&test_db.pool)
                .await
                .unwrap();

        filter.kelas_id = Some(kelas_8);
        let data = fetch_reference_data(&test_db.pool, &filter, &seed.tahun, 1).await;
        assert_eq!(data.tingkat, Some(8));
        assert!(data.categories.is_empty());
        assert!(data.types.is_empty());
        assert_eq!(data.kategori_nilai_id, 0);
        assert_eq!(data.jenis_penilaian_id, 0);
        assert!(data.error.is_none());
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
    async fn sync_recalculates_report_scores_with_laravel_denominator_and_floor() {
        let Some(test_db) = TestDb::try_new().await else {
            return;
        };
        let seed = seed_base_data(&test_db.pool).await;
        let filter = seed_sync_context(&test_db.pool, &seed, 80.0).await;

        add_rapor_score(&test_db.pool, &seed, &filter, 90, "2").await;
        add_rapor_score(&test_db.pool, &seed, &filter, 70, "3").await;
        add_rapor_score(&test_db.pool, &seed, &filter, 85, "4").await;
        sinkron_execute(
            page_context(&seed),
            Extension(test_db.pool.clone()),
            Form(filter.clone()),
        )
        .await;

        let nilai: i32 = sqlx::query_scalar("SELECT nilai FROM nilai_rapors")
            .fetch_one(&test_db.pool)
            .await
            .unwrap();
        assert_eq!(nilai, 81);

        sqlx::query("UPDATE hasil_nilais SET total_nilai = 100 WHERE ujian_id = ?")
            .bind(filter.ujian_id)
            .execute(&test_db.pool)
            .await
            .unwrap();
        sinkron_execute(
            page_context(&seed),
            Extension(test_db.pool.clone()),
            Form(filter),
        )
        .await;
        let updated: i32 = sqlx::query_scalar("SELECT nilai FROM nilai_rapors")
            .fetch_one(&test_db.pool)
            .await
            .unwrap();
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM nilai_rapors")
            .fetch_one(&test_db.pool)
            .await
            .unwrap();
        assert_eq!(updated, 86);
        assert_eq!(count, 1);
        test_db.teardown().await;
    }

    #[tokio::test]
    #[serial]
    async fn sync_uses_all_configured_types_when_scores_are_missing_or_zero() {
        let Some(test_db) = TestDb::try_new().await else {
            return;
        };
        let seed = seed_base_data(&test_db.pool).await;
        let filter = seed_sync_context(&test_db.pool, &seed, 80.0).await;
        add_rapor_score(&test_db.pool, &seed, &filter, 90, "2").await;
        for suffix in ["3", "4"] {
            let category_id = sqlx::query("INSERT INTO kategori_nilais (nama) VALUES (?)")
                .bind(format!("Kategori {suffix}"))
                .execute(&test_db.pool)
                .await
                .unwrap()
                .last_insert_id() as i64;
            let type_id =
                sqlx::query("INSERT INTO jenis_penilaians (nama, kategori_nilai_id) VALUES (?, ?)")
                    .bind(format!("Jenis {suffix}"))
                    .bind(category_id)
                    .execute(&test_db.pool)
                    .await
                    .unwrap()
                    .last_insert_id() as i64;
            sqlx::query("INSERT INTO penilaian_rapors (tingkat, tahun, semester, kategori_nilai_id, jenis_penilaian_id) VALUES (7, ?, '1', ?, ?)")
                .bind(&seed.tahun)
                .bind(category_id)
                .bind(type_id)
                .execute(&test_db.pool)
                .await
                .unwrap();
        }
        // Four configured types but only two submitted scores: 170 / 4.
        sinkron_execute(
            page_context(&seed),
            Extension(test_db.pool.clone()),
            Form(filter),
        )
        .await;
        let nilai: i32 = sqlx::query_scalar("SELECT nilai FROM nilai_rapors")
            .fetch_one(&test_db.pool)
            .await
            .unwrap();
        assert_eq!(nilai, 42);
        test_db.teardown().await;
    }

    #[tokio::test]
    #[serial]
    async fn sync_includes_zero_scores_in_report_calculation() {
        let Some(test_db) = TestDb::try_new().await else {
            return;
        };
        let seed = seed_base_data(&test_db.pool).await;
        let filter = seed_sync_context(&test_db.pool, &seed, 0.0).await;
        add_rapor_score(&test_db.pool, &seed, &filter, 80, "2").await;
        add_rapor_score(&test_db.pool, &seed, &filter, 90, "3").await;
        add_rapor_score(&test_db.pool, &seed, &filter, 70, "4").await;

        sinkron_execute(
            page_context(&seed),
            Extension(test_db.pool.clone()),
            Form(filter),
        )
        .await;
        let nilai: i32 = sqlx::query_scalar("SELECT nilai FROM nilai_rapors")
            .fetch_one(&test_db.pool)
            .await
            .unwrap();
        assert_eq!(nilai, 60);
        test_db.teardown().await;
    }

    #[tokio::test]
    #[serial]
    async fn locked_scores_reject_sync_without_writes() {
        let Some(test_db) = TestDb::try_new().await else {
            return;
        };
        let seed = seed_base_data(&test_db.pool).await;
        let filter = seed_sync_context(&test_db.pool, &seed, 80.0).await;
        sqlx::query(
            "INSERT INTO kunci_nilais (tahun, semester, tingkat, is_active) VALUES (?, '1', 7, 0)",
        )
        .bind(&seed.tahun)
        .execute(&test_db.pool)
        .await
        .unwrap();

        let response = sinkron_execute(
            page_context(&seed),
            Extension(test_db.pool.clone()),
            Form(filter),
        )
        .await;
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM penilaians")
                .fetch_one(&test_db.pool)
                .await
                .unwrap(),
            0
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM nilai_rapors")
                .fetch_one(&test_db.pool)
                .await
                .unwrap(),
            0
        );
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
