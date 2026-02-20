use std::collections::HashMap;

use axum::{
    Extension, Form,
    extract::{Path, Query},
    http::HeaderMap,
    response::{Html, IntoResponse, Redirect, Response},
};
use chrono::{Duration, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, MySqlPool};

use crate::utils::{page_context::PageContext, render::render, tahun::data_tahun};

#[derive(Serialize)]
struct UjianGateData {
    active_ujian: Option<ActiveUjianInfo>,
    message: Option<String>,
    error: Option<String>,
}

#[derive(Serialize)]
struct ActiveUjianInfo {
    ujian_id: i64,
    title: String,
    description: Option<String>,
    waktu_menit: i32,
    total_soal: i32,
    jurusan: String,
    has_session: bool,
    is_submitted: bool,
}

#[derive(FromRow)]
struct ActiveExamRow {
    ujian_id: i64,
    title: String,
    description: Option<String>,
    waktu_menit: i32,
    total_soal: i32,
    jurusan: String,
}

#[derive(FromRow)]
struct PesertaStatusRow {
    id: i64,
    status: String,
}

#[derive(Deserialize)]
pub struct GateQuery {
    pub message: Option<String>,
}

#[derive(Deserialize)]
pub struct TokenForm {
    token: String,
}

#[derive(FromRow)]
struct SiswaProfil {
    jurusan: Option<String>,
}

pub async fn ujian_gate(
    ctx: PageContext,
    Query(query): Query<GateQuery>,
    Extension(db): Extension<MySqlPool>,
) -> Html<String> {
    let Some(nis) = ctx.user.nis.clone() else {
        let data = UjianGateData {
            active_ujian: None,
            message: None,
            error: Some("Akun siswa belum memiliki NIS.".to_string()),
        };
        return render(&ctx, "siswa/ujian/index.html", "Ujian Siswa", data);
    };

    let profil = get_siswa_profil(&db, &nis).await;
    let data = build_gate_data(&db, &nis, profil.as_ref(), query.message, None).await;
    render(&ctx, "siswa/ujian/index.html", "Ujian Siswa", data)
}

pub async fn ujian_konfirmasi_token(
    ctx: PageContext,
    Extension(db): Extension<MySqlPool>,
    Form(payload): Form<TokenForm>,
) -> Response {
    let Some(nis) = ctx.user.nis.clone() else {
        let data = UjianGateData {
            active_ujian: None,
            message: None,
            error: Some("Akun siswa belum memiliki NIS.".to_string()),
        };
        let page = render(&ctx, "siswa/ujian/index.html", "Ujian Siswa", data);
        return Html(page.0).into_response();
    };

    let profil = get_siswa_profil(&db, &nis).await;
    let jurusan = profil
        .as_ref()
        .and_then(|p| p.jurusan.clone())
        .unwrap_or_else(|| "UMUM".to_string());

    let token = payload.token.trim().to_uppercase();
    if token.is_empty() {
        let data = build_gate_data(
            &db,
            &nis,
            profil.as_ref(),
            None,
            Some("Token ujian tidak boleh kosong.".to_string()),
        )
        .await;
        let page = render(&ctx, "siswa/ujian/index.html", "Ujian Siswa", data);
        return Html(page.0).into_response();
    }

    let token_row = sqlx::query_as::<_, TokenLookupRow>(
        r#"
        SELECT
            t.id as token_id,
            t.ujian_id,
            u.title,
            u.jurusan
        FROM ujian_tokens t
        JOIN ujians u ON u.id = t.ujian_id
        WHERE t.token = ?
          AND t.is_active = 1
          AND u.is_active = 1
          AND (t.expired_at IS NULL OR t.expired_at > NOW())
          AND (u.jurusan = 'UMUM' OR u.jurusan = ?)
        LIMIT 1
        "#,
    )
    .bind(&token)
    .bind(&jurusan)
    .fetch_optional(&db)
    .await
    .unwrap_or(None);

    let Some(token_row) = token_row else {
        let data = build_gate_data(
            &db,
            &nis,
            profil.as_ref(),
            None,
            Some("Token tidak valid, tidak aktif, atau tidak sesuai jurusan Anda.".to_string()),
        )
        .await;
        let page = render(&ctx, "siswa/ujian/index.html", "Ujian Siswa", data);
        return Html(page.0).into_response();
    };

    let peserta_existing = sqlx::query_as::<_, PesertaStatusRow>(
        r#"
        SELECT id, status
        FROM ujian_pesertas
        WHERE ujian_id = ? AND nis = ?
        LIMIT 1
        "#,
    )
    .bind(token_row.ujian_id)
    .bind(&nis)
    .fetch_optional(&db)
    .await
    .unwrap_or(None);

    if let Some(existing) = peserta_existing {
        if existing.status == "submitted" {
            return Redirect::to(
                "/siswa/ujian?message=Anda+sudah+menyelesaikan+ujian+ini",
            )
            .into_response();
        }
        return Redirect::to(&format!("/siswa/ujian/{}", existing.id)).into_response();
    }

    let inserted = sqlx::query(
        r#"
        INSERT INTO ujian_pesertas
            (ujian_id, nis, token_id, status, started_at, last_nomor, created_at, updated_at)
        VALUES
            (?, ?, ?, 'started', NOW(), 1, NOW(), NOW())
        "#,
    )
    .bind(token_row.ujian_id)
    .bind(&nis)
    .bind(token_row.token_id)
    .execute(&db)
    .await;

    match inserted {
        Ok(res) => {
            let peserta_id = res.last_insert_id() as i64;
            Redirect::to(&format!("/siswa/ujian/{}", peserta_id)).into_response()
        }
        Err(_) => {
            let data = build_gate_data(
                &db,
                &nis,
                profil.as_ref(),
                None,
                Some(format!(
                    "Gagal memulai ujian {}. Silakan coba lagi.",
                    token_row.title
                )),
            )
            .await;
            let page = render(&ctx, "siswa/ujian/index.html", "Ujian Siswa", data);
            Html(page.0).into_response()
        }
    }
}

#[derive(FromRow)]
struct TokenLookupRow {
    token_id: i64,
    ujian_id: i64,
    title: String,
}

#[derive(Deserialize)]
pub struct SessionQuery {
    nomor: Option<i32>,
}

#[derive(Serialize)]
struct UjianSessionData {
    peserta_id: i64,
    ujian_id: i64,
    ujian_title: String,
    waktu_menit: i32,
    nomor_saat_ini: i32,
    total_soal: i32,
    timer_deadline_ms: i64,
    current: Option<SoalView>,
    nav: Vec<SoalNav>,
}

#[derive(Serialize)]
struct SoalView {
    soal_id: i64,
    pertanyaan: String,
    kategori: Option<String>,
    opsi_a: String,
    opsi_b: String,
    opsi_c: String,
    opsi_d: String,
    jawaban_terpilih: Option<String>,
    jawaban_uraian: Option<String>,
}

#[derive(Serialize)]
struct SoalNav {
    nomor: i32,
    aktif: bool,
    terjawab: bool,
}

#[derive(FromRow)]
struct SessionHeaderRow {
    ujian_id: i64,
    ujian_title: String,
    waktu_menit: i32,
    started_at: NaiveDateTime,
    last_nomor: i32,
    status: String,
}

#[derive(FromRow)]
struct SoalSessionRow {
    soal_id: i64,
    urutan: i32,
    pertanyaan: String,
    kategori: Option<String>,
    opsi_a: Option<String>,
    opsi_b: Option<String>,
    opsi_c: Option<String>,
    opsi_d: Option<String>,
}

#[derive(FromRow)]
struct JawabanRow {
    soal_id: i64,
    pilihan: Option<String>,
    jawaban_uraian: Option<String>,
}

pub async fn ujian_session_page(
    ctx: PageContext,
    Path(peserta_id): Path<i64>,
    Query(query): Query<SessionQuery>,
    Extension(db): Extension<MySqlPool>,
) -> Response {
    let Some(nis) = ctx.user.nis.clone() else {
        return Redirect::to("/siswa/ujian?message=Akun+siswa+belum+memiliki+NIS").into_response();
    };

    let header = sqlx::query_as::<_, SessionHeaderRow>(
        r#"
        SELECT
            p.ujian_id,
            u.title as ujian_title,
            u.waktu_menit,
            p.started_at,
            p.last_nomor,
            p.status
        FROM ujian_pesertas p
        JOIN ujians u ON u.id = p.ujian_id
        WHERE p.id = ? AND p.nis = ?
        LIMIT 1
        "#,
    )
    .bind(peserta_id)
    .bind(&nis)
    .fetch_optional(&db)
    .await
    .unwrap_or(None);

    let Some(header) = header else {
        return Redirect::to("/siswa/ujian?message=Sesi+ujian+tidak+ditemukan").into_response();
    };

    if header.status == "submitted" {
        return Redirect::to(
            "/siswa/ujian?message=Anda+sudah+menyelesaikan+ujian+ini",
        )
        .into_response();
    }

    let deadline = header.started_at + Duration::minutes(header.waktu_menit as i64);
    if Utc::now().naive_utc() > deadline {
        let _ = finalize_submission(&db, header.ujian_id, &nis, peserta_id).await;
        return Redirect::to(
            "/siswa/ujian?message=Waktu+ujian+habis.+Jawaban+otomatis+disubmit",
        )
        .into_response();
    }

    let soals = sqlx::query_as::<_, SoalSessionRow>(
        r#"
        SELECT
            s.id as soal_id,
            us.urutan,
            s.pertanyaan,
            s.kategori,
            s.opsi_a,
            s.opsi_b,
            s.opsi_c,
            s.opsi_d
        FROM ujian_soals us
        JOIN soals s ON s.id = us.soal_id
        WHERE us.ujian_id = ?
        ORDER BY us.urutan ASC
        "#,
    )
    .bind(header.ujian_id)
    .fetch_all(&db)
    .await
    .unwrap_or_default();

    if soals.is_empty() {
        return Redirect::to("/siswa/ujian?message=Ujian+belum+memiliki+soal").into_response();
    }

    let jawaban = sqlx::query_as::<_, JawabanRow>(
        "SELECT soal_id, pilihan, jawaban_uraian FROM ujian_jawabans WHERE ujian_id = ? AND nis = ?",
    )
    .bind(header.ujian_id)
    .bind(&nis)
    .fetch_all(&db)
    .await
    .unwrap_or_default();

    let jawaban_map: HashMap<i64, JawabanRow> =
        jawaban.into_iter().map(|j| (j.soal_id, j)).collect();
    let total_soal = soals.len() as i32;
    let nomor_req = query.nomor.unwrap_or(header.last_nomor);
    let nomor_saat_ini = nomor_req.clamp(1, total_soal);

    let _ = sqlx::query("UPDATE ujian_pesertas SET last_nomor = ?, updated_at = NOW() WHERE id = ?")
        .bind(nomor_saat_ini)
        .bind(peserta_id)
        .execute(&db)
        .await;

    let current_soal = soals
        .iter()
        .find(|s| s.urutan == nomor_saat_ini)
        .or_else(|| soals.first());

    let current = current_soal.map(|s| SoalView {
        soal_id: s.soal_id,
        pertanyaan: s.pertanyaan.clone(),
        kategori: s.kategori.clone(),
        opsi_a: s.opsi_a.clone().unwrap_or_default(),
        opsi_b: s.opsi_b.clone().unwrap_or_default(),
        opsi_c: s.opsi_c.clone().unwrap_or_default(),
        opsi_d: s.opsi_d.clone().unwrap_or_default(),
        jawaban_terpilih: jawaban_map
            .get(&s.soal_id)
            .and_then(|j| j.pilihan.clone()),
        jawaban_uraian: jawaban_map
            .get(&s.soal_id)
            .and_then(|j| j.jawaban_uraian.clone()),
    });

    let nav = soals
        .iter()
        .map(|s| SoalNav {
            nomor: s.urutan,
            aktif: s.urutan == nomor_saat_ini,
            terjawab: jawaban_map
                .get(&s.soal_id)
                .map(|j| {
                    if s.kategori.as_deref() == Some("Uraian") {
                        j.jawaban_uraian
                            .as_deref()
                            .map(|v| !v.trim().is_empty())
                            .unwrap_or(false)
                    } else {
                        j.pilihan.is_some()
                    }
                })
                .unwrap_or(false),
        })
        .collect::<Vec<_>>();

    let data = UjianSessionData {
        peserta_id,
        ujian_id: header.ujian_id,
        ujian_title: header.ujian_title,
        waktu_menit: header.waktu_menit,
        nomor_saat_ini,
        total_soal,
        timer_deadline_ms: deadline.and_utc().timestamp_millis(),
        current,
        nav,
    };

    render(&ctx, "siswa/ujian/session.html", "Sesi Ujian", data).into_response()
}

#[derive(Deserialize)]
pub struct JawabForm {
    soal_id: i64,
    pilihan: Option<String>,
    jawaban_uraian: Option<String>,
    nomor_tujuan: Option<i32>,
    nomor_saat_ini: i32,
}

#[derive(FromRow)]
struct SoalKunciRow {
    kunci_jawaban: Option<String>,
    bobot_nilai: i32,
    kategori: Option<String>,
}

pub async fn ujian_simpan_jawaban(
    ctx: PageContext,
    Path(peserta_id): Path<i64>,
    Extension(db): Extension<MySqlPool>,
    Form(form): Form<JawabForm>,
) -> Response {
    let Some(nis) = ctx.user.nis.clone() else {
        return Redirect::to("/siswa/ujian?message=Akun+siswa+belum+memiliki+NIS").into_response();
    };

    let peserta = sqlx::query_as::<_, PesertaOwnerRow>(
        "SELECT ujian_id, status FROM ujian_pesertas WHERE id = ? AND nis = ? LIMIT 1",
    )
    .bind(peserta_id)
    .bind(&nis)
    .fetch_optional(&db)
    .await
    .unwrap_or(None);

    let Some(peserta) = peserta else {
        return Redirect::to("/siswa/ujian?message=Sesi+ujian+tidak+ditemukan").into_response();
    };

    if peserta.status == "submitted" {
        return Redirect::to(
            "/siswa/ujian?message=Ujian+sudah+disubmit+dan+tidak+dapat+diubah",
        )
        .into_response();
    }

    simpan_jawaban_opsional(
        &db,
        peserta.ujian_id,
        &nis,
        form.soal_id,
        form.pilihan.clone(),
        form.jawaban_uraian.clone(),
    )
    .await;

    let nomor_tujuan = form.nomor_tujuan.unwrap_or(form.nomor_saat_ini).max(1);
    let _ = sqlx::query("UPDATE ujian_pesertas SET last_nomor = ?, updated_at = NOW() WHERE id = ?")
        .bind(nomor_tujuan)
        .bind(peserta_id)
        .execute(&db)
        .await;

    let mut headers = HeaderMap::new();
    headers.insert(
        "HX-Redirect",
        format!("/siswa/ujian/{}?nomor={}", peserta_id, nomor_tujuan)
            .parse()
            .unwrap(),
    );
    headers.into_response()
}

#[derive(FromRow)]
struct PesertaOwnerRow {
    ujian_id: i64,
    status: String,
}

#[derive(Deserialize)]
pub struct SubmitForm {
    #[serde(default)]
    _konfirmasi: Option<String>,
    soal_id: Option<i64>,
    pilihan: Option<String>,
    jawaban_uraian: Option<String>,
}

pub async fn ujian_submit(
    ctx: PageContext,
    Path(peserta_id): Path<i64>,
    Extension(db): Extension<MySqlPool>,
    Form(form): Form<SubmitForm>,
) -> Response {
    let Some(nis) = ctx.user.nis.clone() else {
        return Redirect::to("/siswa/ujian?message=Akun+siswa+belum+memiliki+NIS").into_response();
    };

    let peserta = sqlx::query_as::<_, PesertaOwnerRow>(
        "SELECT ujian_id, status FROM ujian_pesertas WHERE id = ? AND nis = ? LIMIT 1",
    )
    .bind(peserta_id)
    .bind(&nis)
    .fetch_optional(&db)
    .await
    .unwrap_or(None);

    let Some(peserta) = peserta else {
        return Redirect::to("/siswa/ujian?message=Sesi+ujian+tidak+ditemukan").into_response();
    };

    if peserta.status == "submitted" {
        return Redirect::to(
            "/siswa/ujian?message=Anda+sudah+menyelesaikan+ujian+ini",
        )
        .into_response();
    }

    if let Some(soal_id) = form.soal_id {
        simpan_jawaban_opsional(
            &db,
            peserta.ujian_id,
            &nis,
            soal_id,
            form.pilihan.clone(),
            form.jawaban_uraian.clone(),
        )
        .await;
    }

    let _ = finalize_submission(&db, peserta.ujian_id, &nis, peserta_id).await;

    Redirect::to(
        "/siswa/ujian?message=Ujian+berhasil+disubmit.+Nilai+akhir+akan+diproses+di+dashboard+guru/admin",
    )
    .into_response()
}

async fn finalize_submission(db: &MySqlPool, ujian_id: i64, nis: &str, peserta_id: i64) -> i64 {
    let total_nilai: i64 = sqlx::query_scalar(
        r#"
        SELECT COALESCE(
            SUM(
                CASE
                    WHEN j.pilihan = s.kunci_jawaban THEN COALESCE(s.bobot_nilai, 1)
                    ELSE 0
                END
            ),
            0
        ) AS nilai_total
        FROM ujian_jawabans j
        JOIN soals s ON s.id = j.soal_id
        WHERE j.ujian_id = ? AND j.nis = ?
        "#,
    )
    .bind(ujian_id)
    .bind(nis)
    .fetch_one(db)
    .await
    .unwrap_or(0);

    let _ = sqlx::query(
        r#"
        UPDATE ujian_pesertas
        SET status = 'submitted',
            submitted_at = NOW(),
            total_nilai = ?,
            updated_at = NOW()
        WHERE id = ?
        "#,
    )
    .bind(total_nilai as i32)
    .bind(peserta_id)
    .execute(db)
    .await;

    total_nilai
}

async fn simpan_jawaban_opsional(
    db: &MySqlPool,
    ujian_id: i64,
    nis: &str,
    soal_id: i64,
    pilihan: Option<String>,
    jawaban_uraian: Option<String>,
) {
    let kunci = sqlx::query_as::<_, SoalKunciRow>(
        r#"
        SELECT s.kunci_jawaban, COALESCE(s.bobot_nilai, 1) as bobot_nilai, s.kategori
        FROM ujian_soals us
        JOIN soals s ON s.id = us.soal_id
        WHERE us.ujian_id = ? AND us.soal_id = ?
        LIMIT 1
        "#,
    )
    .bind(ujian_id)
    .bind(soal_id)
    .fetch_optional(db)
    .await
    .unwrap_or(None);

    let Some(k) = kunci else {
        return;
    };

    let is_uraian = k.kategori.as_deref() == Some("Uraian");
    if is_uraian {
        let Some(text) = jawaban_uraian.as_deref() else {
            return;
        };
        if text.trim().is_empty() {
            return;
        }

        let _ = sqlx::query(
            r#"
            INSERT INTO ujian_jawabans
                (ujian_id, nis, soal_id, pilihan, jawaban_uraian, is_benar, bobot_nilai, created_at, updated_at)
            VALUES
                (?, ?, ?, NULL, ?, FALSE, 0, NOW(), NOW())
            ON DUPLICATE KEY UPDATE
                jawaban_uraian = VALUES(jawaban_uraian),
                pilihan = NULL,
                is_benar = FALSE,
                bobot_nilai = 0,
                updated_at = NOW()
            "#,
        )
        .bind(ujian_id)
        .bind(nis)
        .bind(soal_id)
        .bind(text)
        .execute(db)
        .await;
        return;
    }

    let Some(pilihan_raw) = pilihan else {
        return;
    };

    let pilihan = pilihan_raw.to_lowercase();
    if !matches!(pilihan.as_str(), "a" | "b" | "c" | "d") {
        return;
    }

    let is_benar = k
        .kunci_jawaban
        .as_deref()
        .map(|v| v.eq_ignore_ascii_case(&pilihan))
        .unwrap_or(false);
    let nilai = if is_benar { k.bobot_nilai } else { 0 };

    let _ = sqlx::query(
        r#"
        INSERT INTO ujian_jawabans
            (ujian_id, nis, soal_id, pilihan, jawaban_uraian, is_benar, bobot_nilai, created_at, updated_at)
        VALUES
            (?, ?, ?, ?, NULL, ?, ?, NOW(), NOW())
        ON DUPLICATE KEY UPDATE
            pilihan = VALUES(pilihan),
            jawaban_uraian = NULL,
            is_benar = VALUES(is_benar),
            bobot_nilai = VALUES(bobot_nilai),
            updated_at = NOW()
        "#,
    )
    .bind(ujian_id)
    .bind(nis)
    .bind(soal_id)
    .bind(pilihan)
    .bind(is_benar)
    .bind(nilai)
    .execute(db)
    .await;
}

async fn build_gate_data(
    db: &MySqlPool,
    nis: &str,
    profil: Option<&SiswaProfil>,
    message: Option<String>,
    error: Option<String>,
) -> UjianGateData {
    let jurusan = profil
        .and_then(|p| p.jurusan.clone())
        .unwrap_or_else(|| "UMUM".to_string());

    let active_ujian = sqlx::query_as::<_, ActiveExamRow>(
        r#"
        SELECT
            u.id as ujian_id,
            u.title,
            u.description,
            u.waktu_menit,
            u.total_soal,
            u.jurusan
        FROM ujians u
        JOIN ujian_tokens t ON t.ujian_id = u.id
        WHERE u.is_active = 1
          AND t.is_active = 1
          AND (t.expired_at IS NULL OR t.expired_at > NOW())
          AND (u.jurusan = 'UMUM' OR u.jurusan = ?)
        ORDER BY t.created_at DESC
        LIMIT 1
        "#,
    )
    .bind(&jurusan)
    .fetch_optional(db)
    .await
    .unwrap_or(None);

    let active_ujian = if let Some(exam) = active_ujian {
        let peserta = sqlx::query_as::<_, PesertaStatusRow>(
            "SELECT id, status FROM ujian_pesertas WHERE ujian_id = ? AND nis = ? LIMIT 1",
        )
        .bind(exam.ujian_id)
        .bind(nis)
        .fetch_optional(db)
        .await
        .unwrap_or(None);

        Some(ActiveUjianInfo {
            ujian_id: exam.ujian_id,
            title: exam.title,
            description: exam.description,
            waktu_menit: exam.waktu_menit,
            total_soal: exam.total_soal,
            jurusan: exam.jurusan,
            has_session: peserta.is_some(),
            is_submitted: peserta
                .as_ref()
                .map(|p| p.status == "submitted")
                .unwrap_or(false),
        })
    } else {
        None
    };

    UjianGateData {
        active_ujian,
        message,
        error,
    }
}

async fn get_siswa_profil(db: &MySqlPool, nis: &str) -> Option<SiswaProfil> {
    let tahun = data_tahun();

    let by_tahun = sqlx::query_as::<_, SiswaProfil>(
        r#"
        SELECT s.nis, k.kode_keahlian as jurusan
        FROM siswas s
        JOIN kelas k ON k.id = s.kelas_id
        WHERE s.nis = ? AND s.tahun = ?
        LIMIT 1
        "#,
    )
    .bind(nis)
    .bind(&tahun)
    .fetch_optional(db)
    .await
    .unwrap_or(None);

    if by_tahun.is_some() {
        return by_tahun;
    }

    sqlx::query_as::<_, SiswaProfil>(
        r#"
        SELECT s.nis, k.kode_keahlian as jurusan
        FROM siswas s
        JOIN kelas k ON k.id = s.kelas_id
        WHERE s.nis = ?
        ORDER BY s.id DESC
        LIMIT 1
        "#,
    )
    .bind(nis)
    .fetch_optional(db)
    .await
    .unwrap_or(None)
}
