use std::collections::HashMap;

use axum::{
    Extension, Form,
    extract::{Path, Query},
    http::HeaderMap,
    response::{Html, IntoResponse, Redirect, Response},
};
use bytes::Bytes;
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

    let data = build_gate_data(&db, &nis, query.message, None).await;
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

    let token = payload.token.trim().to_uppercase();
    if token.is_empty() {
        let data = build_gate_data(
            &db,
            &nis,
            None,
            Some("Token ujian tidak boleh kosong.".to_string()),
        )
        .await;
        let page = render(&ctx, "siswa/ujian/index.html", "Ujian Siswa", data);
        return Html(page.0).into_response();
    }

    let tahun = data_tahun();
    let token_row = sqlx::query_as::<_, TokenLookupRow>(
        r#"
        SELECT
            t.id as token_id,
            t.ujian_id,
            u.title,
            t.tahun,
            t.lab_kode,
            CAST(t.sesi AS SIGNED) as sesi,
            CAST(t.gelombang AS SIGNED) as gelombang
        FROM ujian_tokens t
        JOIN ujians u ON u.id = t.ujian_id
        WHERE t.token = ?
          AND t.is_active = 1
          AND u.is_active = 1
          AND (t.expired_at IS NULL OR t.expired_at > NOW())
          AND u.tahun = ?
        LIMIT 1
        "#,
    )
    .bind(&token)
    .bind(&tahun)
    .fetch_optional(&db)
    .await
    .unwrap_or(None);

    let Some(token_row) = token_row else {
        let data = build_gate_data(
            &db,
            &nis,
            None,
            Some("Token tidak valid atau tidak aktif.".to_string()),
        )
        .await;
        let page = render(&ctx, "siswa/ujian/index.html", "Ujian Siswa", data);
        return Html(page.0).into_response();
    };

    let peserta_terdaftar = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT id
        FROM ujian_pesertas
        WHERE nis = ?
          AND tahun = ?
          AND lab_kode = ?
          AND sesi = ?
          AND gelombang = ?
        LIMIT 1
        "#,
    )
    .bind(&nis)
    .bind(token_row.tahun.as_deref().unwrap_or(&tahun))
    .bind(token_row.lab_kode.as_deref().unwrap_or("01"))
    .bind(token_row.sesi.unwrap_or(1))
    .bind(token_row.gelombang.unwrap_or(1))
    .fetch_optional(&db)
    .await
    .unwrap_or(None);

    if peserta_terdaftar.is_none() {
        let data = build_gate_data(
            &db,
            &nis,
            None,
            Some("Anda belum terdaftar pada grup ujian sesuai token tersebut.".to_string()),
        )
        .await;
        let page = render(&ctx, "siswa/ujian/index.html", "Ujian Siswa", data);
        return Html(page.0).into_response();
    }

    let peserta_existing = sqlx::query_as::<_, PesertaStatusRow>(
        r#"
        SELECT id, status
        FROM ujian_pengerjaans
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
            return Redirect::to("/siswa/ujian?message=Anda+sudah+menyelesaikan+ujian+ini")
                .into_response();
        }
        return Redirect::to(&format!("/siswa/ujian/{}", existing.id)).into_response();
    }

    let kelas_id = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT CAST(kelas_id AS SIGNED) as kelas_id
        FROM ujian_pesertas
        WHERE nis = ?
          AND tahun = ?
          AND lab_kode = ?
          AND sesi = ?
          AND gelombang = ?
        LIMIT 1
        "#,
    )
    .bind(&nis)
    .bind(token_row.tahun.as_deref().unwrap_or(&tahun))
    .bind(token_row.lab_kode.as_deref().unwrap_or("01"))
    .bind(token_row.sesi.unwrap_or(1))
    .bind(token_row.gelombang.unwrap_or(1))
    .fetch_optional(&db)
    .await
    .unwrap_or(None);

    let inserted = sqlx::query(
        r#"
        INSERT INTO ujian_pengerjaans
            (ujian_id, nis, token_id, tahun, kelas_id, lab_kode, sesi, gelombang, status, started_at, last_nomor, created_at, updated_at)
        VALUES
            (?, ?, ?, ?, ?, ?, ?, ?, 'started', NOW(), 1, NOW(), NOW())
        "#,
    )
    .bind(token_row.ujian_id)
    .bind(&nis)
    .bind(token_row.token_id)
    .bind(token_row.tahun)
    .bind(kelas_id)
    .bind(token_row.lab_kode)
    .bind(token_row.sesi)
    .bind(token_row.gelombang)
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
    tahun: Option<String>,
    lab_kode: Option<String>,
    sesi: Option<i32>,
    gelombang: Option<i32>,
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
struct TrueFalseStatementView {
    label: String,
    text: String,
    jawaban: Option<String>,
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
    pernyataan_bs: Vec<TrueFalseStatementView>,
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
        FROM ujian_pengerjaans p
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
        return Redirect::to("/siswa/ujian?message=Anda+sudah+menyelesaikan+ujian+ini")
            .into_response();
    }

    let deadline = header.started_at + Duration::minutes(header.waktu_menit as i64);
    if Utc::now().naive_utc() > deadline {
        eprintln!(
            "DEBUG ujian_session_page: waktu habis, auto finalize - peserta_id={}, ujian_id={}, nis={}",
            peserta_id, header.ujian_id, nis
        );
        let _ = finalize_submission(&db, header.ujian_id, &nis, peserta_id).await;
        return Redirect::to("/siswa/ujian?message=Waktu+ujian+habis.+Jawaban+otomatis+disubmit")
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

    let _ =
        sqlx::query("UPDATE ujian_pengerjaans SET last_nomor = ?, updated_at = NOW() WHERE id = ?")
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
        pernyataan_bs: build_true_false_statements(
            [
                s.opsi_a.as_deref(),
                s.opsi_b.as_deref(),
                s.opsi_c.as_deref(),
            ],
            jawaban_map
                .get(&s.soal_id)
                .and_then(|j| j.pilihan.as_deref()),
        ),
        jawaban_terpilih: jawaban_map.get(&s.soal_id).and_then(|j| j.pilihan.clone()),
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

#[derive(FromRow)]
struct SoalKunciRow {
    kunci_jawaban: Option<String>,
    bobot_nilai: f64,
    kategori: Option<String>,
}

pub async fn ujian_simpan_jawaban(
    ctx: PageContext,
    Path(peserta_id): Path<i64>,
    Extension(db): Extension<MySqlPool>,
    body: Bytes,
) -> Response {
    // parse form body into a map that preserves repeated keys
    // parse form body into a vector of pairs to preserve repeated keys, then build a map
    let mut form_map: HashMap<String, Vec<String>> = HashMap::new();
    match serde_urlencoded::from_bytes::<Vec<(String, String)>>(&body) {
        Ok(pairs) => {
            for (k, v) in pairs {
                form_map.entry(k).or_default().push(v);
            }
        }
        Err(e) => {
            eprintln!(
                "ERROR ujian_simpan_jawaban: failed to parse form body: {:?}",
                e
            );
        }
    }
    eprintln!(
        "DEBUG ujian_simpan_jawaban: raw_form_keys={:?}",
        form_map.keys().collect::<Vec<_>>()
    );

    let Some(nis) = ctx.user.nis.clone() else {
        return Redirect::to("/siswa/ujian?message=Akun+siswa+belum+memiliki+NIS").into_response();
    };

    let peserta = sqlx::query_as::<_, PesertaOwnerRow>(
        "SELECT ujian_id, status FROM ujian_pengerjaans WHERE id = ? AND nis = ? LIMIT 1",
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
        return Redirect::to("/siswa/ujian?message=Ujian+sudah+disubmit+dan+tidak+dapat+diubah")
            .into_response();
    }

    // manually extract form fields
    let soal_id_opt = form_map
        .get("soal_id")
        .and_then(|v| v.first())
        .and_then(|s| s.parse::<i64>().ok());
    let soal_id = match soal_id_opt {
        Some(id) => id,
        None => {
            return Redirect::to("/siswa/ujian?message=Soal+tidak+teridentifikasi").into_response();
        }
    };

    // pilihan can be provided as repeated `pilihan` fields, or a single `pilihan` value, or `pilihan[]` keys
    let mut pilihan_vals: Vec<String> = Vec::new();
    if let Some(v) = form_map.get("pilihan") {
        pilihan_vals.extend(v.iter().cloned());
    }
    if let Some(v) = form_map.get("pilihan[]") {
        pilihan_vals.extend(v.iter().cloned());
    }
    let mut pilihan_bs_keys = form_map
        .keys()
        .filter(|key| key.starts_with("pilihan_bs_"))
        .cloned()
        .collect::<Vec<_>>();
    pilihan_bs_keys.sort();
    for key in pilihan_bs_keys {
        if let Some(values) = form_map.get(&key).and_then(|v| v.first()) {
            pilihan_vals.push(values.clone());
        }
    }

    let pilihan_joined = if pilihan_vals.is_empty() {
        None
    } else {
        let vec = pilihan_vals
            .iter()
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>();
        if vec.is_empty() {
            None
        } else {
            Some(vec.join(","))
        }
    };

    let jawaban_uraian = form_map
        .get("jawaban_uraian")
        .and_then(|v| v.first())
        .cloned();
    let nomor_tujuan = form_map
        .get("nomor_tujuan")
        .and_then(|v| v.first())
        .and_then(|s| s.parse::<i32>().ok());
    let nomor_saat_ini = form_map
        .get("nomor_saat_ini")
        .and_then(|v| v.first())
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(1);

    eprintln!(
        "DEBUG ujian_simpan_jawaban: peserta_id={}, ujian_id={}, soal_id={}, pilihan={:?}, jawaban_uraian={:?}",
        peserta_id, peserta.ujian_id, soal_id, pilihan_joined, jawaban_uraian
    );

    simpan_jawaban_opsional(
        &db,
        peserta.ujian_id,
        &nis,
        soal_id,
        pilihan_joined,
        jawaban_uraian,
    )
    .await;
    eprintln!(
        "DEBUG ujian_simpan_jawaban: finished save attempt for peserta_id={}, soal_id={}",
        peserta_id, soal_id
    );

    let nomor_tujuan = nomor_tujuan.unwrap_or(nomor_saat_ini).max(1);
    let _ =
        sqlx::query("UPDATE ujian_pengerjaans SET last_nomor = ?, updated_at = NOW() WHERE id = ?")
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
    eprintln!(
        "DEBUG ujian_submit: peserta_id={}, form.soal_id={:?}, form.pilihan={:?}, form.jawaban_uraian={:?}",
        peserta_id, form.soal_id, form.pilihan, form.jawaban_uraian
    );
    eprintln!(
        "DEBUG ujian_submit: payload meta - konfirmasi={:?}, pilihan_len={}, jawaban_uraian_len={}",
        form._konfirmasi,
        form.pilihan.as_ref().map(|v| v.len()).unwrap_or(0),
        form.jawaban_uraian.as_ref().map(|v| v.len()).unwrap_or(0)
    );

    let Some(nis) = ctx.user.nis.clone() else {
        return Redirect::to("/siswa/ujian?message=Akun+siswa+belum+memiliki+NIS").into_response();
    };

    let peserta = sqlx::query_as::<_, PesertaOwnerRow>(
        "SELECT ujian_id, status FROM ujian_pengerjaans WHERE id = ? AND nis = ? LIMIT 1",
    )
    .bind(peserta_id)
    .bind(&nis)
    .fetch_optional(&db)
    .await
    .unwrap_or(None);

    let Some(peserta) = peserta else {
        return Redirect::to("/siswa/ujian?message=Sesi+ujian+tidak+ditemukan").into_response();
    };

    eprintln!(
        "DEBUG ujian_submit: peserta found - ujian_id={}, status={}",
        peserta.ujian_id, peserta.status
    );

    if peserta.status == "submitted" {
        return Redirect::to("/siswa/ujian?message=Anda+sudah+menyelesaikan+ujian+ini")
            .into_response();
    }

    if let Some(soal_id) = form.soal_id {
        eprintln!(
            "DEBUG ujian_submit: akan menyimpan jawaban soal_id={}",
            soal_id
        );
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

    eprintln!(
        "DEBUG ujian_submit: akan memanggil finalize_submission untuk ujian_id={}, nis={}",
        peserta.ujian_id, nis
    );
    let total_nilai = finalize_submission(&db, peserta.ujian_id, &nis, peserta_id).await;
    eprintln!(
        "DEBUG ujian_submit: finalize_submission selesai, total_nilai={}",
        total_nilai
    );

    Redirect::to(
        "/siswa/ujian?message=Ujian+berhasil+disubmit.+Nilai+akhir+akan+diproses+di+dashboard+guru/admin",
    )
    .into_response()
}

async fn finalize_submission(db: &MySqlPool, ujian_id: i64, nis: &str, peserta_id: i64) -> f64 {
    eprintln!(
        "DEBUG finalize_submission: START - ujian_id={}, nis={}, peserta_id={}",
        ujian_id, nis, peserta_id
    );

    // Fetch ujian metadata (mata_pelajaran and tahun)
    let ujian_meta: Option<(i64, Option<String>)> = sqlx::query_as(
        r#"
        SELECT CAST(mata_pelajaran_id AS SIGNED) as mata_pelajaran_id, tahun
        FROM ujians
        WHERE id = ?
        LIMIT 1
        "#,
    )
    .bind(ujian_id)
    .fetch_optional(db)
    .await
    .unwrap_or_else(|e| {
        eprintln!(
            "DEBUG finalize_submission: hasil_nilais meta query failed: ujian_id={}, peserta_id={}, nis={}, err={:?}",
            ujian_id, peserta_id, nis, e
        );
        None
    });

    eprintln!("DEBUG finalize_submission: ujian_meta={:?}", ujian_meta);
    if let Some((_, None)) = ujian_meta {
        eprintln!(
            "WARN finalize_submission: ujian_meta.tahun is NULL - ujian_id={}, nis={}, peserta_id={}",
            ujian_id, nis, peserta_id
        );
    }

    // If we have ujian metadata, compute totals directly from existing answers in ujian_jawabans
    if let Some((mata_pelajaran_id, Some(tahun))) = ujian_meta {
        eprintln!(
            "DEBUG finalize_submission: mata_pelajaran_id={}, tahun={}",
            mata_pelajaran_id, tahun
        );

        // Check if there are any answers in ujian_jawabans for this nis and ujian_id
        let answer_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM ujian_jawabans WHERE ujian_id = ? AND nis = ?",
        )
        .bind(ujian_id)
        .bind(nis)
        .fetch_one(db)
        .await
        .unwrap_or(0);
        eprintln!(
            "DEBUG finalize_submission: answer_count for this ujian_id and nis = {}",
            answer_count
        );

        // total_benar: sum ALL bobot_nilai from ujian_jawabans
        // This includes partial credit for Pilihan Ganda Kompleks (already calculated in simpan_jawaban_opsional)
        // Note: We sum ALL bobot_nilai (not just is_benar=1) because bobot_nilai already contains the earned score
        eprintln!(
            "DEBUG finalize_submission: about to query total_benar with ujian_id={}, nis={}",
            ujian_id, nis
        );

        // Fix: Cast DECIMAL to DOUBLE to avoid type mismatch in sqlx
        // Sum ALL bobot_nilai (not filtering by is_benar)
        let total_benar_result: Result<f64, _> = sqlx::query_scalar(
            r#"
            SELECT COALESCE(SUM(CAST(COALESCE(j.bobot_nilai, 0) AS DOUBLE)), 0)
            FROM ujian_jawabans j
            WHERE j.ujian_id = ?
              AND j.nis = ?
            "#,
        )
        .bind(ujian_id)
        .bind(nis)
        .fetch_one(db)
        .await;

        let total_benar = match total_benar_result {
            Ok(val) => {
                eprintln!(
                    "DEBUG finalize_submission: total_benar query SUCCESS, value={}",
                    val
                );
                val
            }
            Err(e) => {
                eprintln!(
                    "DEBUG finalize_submission: total_benar query FAILED: {:?}",
                    e
                );
                0.0
            }
        };

        eprintln!(
            "DEBUG finalize_submission: total_benar (sum of all bobot_nilai from ujian_jawabans) = {}",
            total_benar
        );

        // total_possible: sum bobot_nilai of all questions in this ujian
        eprintln!(
            "DEBUG finalize_submission: about to query total_possible with ujian_id={}",
            ujian_id
        );

        // Fix: Cast DECIMAL to DOUBLE to avoid type mismatch in sqlx
        let total_possible_result: Result<f64, _> = sqlx::query_scalar(
            r#"
            SELECT COALESCE(SUM(CAST(COALESCE(s.bobot_nilai, 0) AS DOUBLE)), 0)
            FROM ujian_soals us
            JOIN soals s ON s.id = us.soal_id
            WHERE us.ujian_id = ?
            "#,
        )
        .bind(ujian_id)
        .fetch_one(db)
        .await;

        let total_possible = match total_possible_result {
            Ok(val) => {
                eprintln!(
                    "DEBUG finalize_submission: total_possible query SUCCESS, value={}",
                    val
                );
                val
            }
            Err(e) => {
                eprintln!(
                    "DEBUG finalize_submission: total_possible query FAILED: {:?}",
                    e
                );
                0.0
            }
        };

        eprintln!(
            "DEBUG finalize_submission: total_possible = {}",
            total_possible
        );

        // total_salah = total_possible - total_benar (clamp >= 0)
        let mut total_salah = total_possible - total_benar;
        if total_salah < 0.0 {
            total_salah = 0.0;
        }

        eprintln!("DEBUG finalize_submission: total_salah = {}", total_salah);

        // total_uraian: sum nilai_uraian for uraian answers (typically reviewed manually later)
        // Fix: Cast DECIMAL to DOUBLE to avoid type mismatch in sqlx
        let total_uraian: Result<f64, _> = sqlx::query_scalar(
            r#"
            SELECT COALESCE(SUM(CAST(COALESCE(j.nilai_uraian, 0) AS DOUBLE)), 0)
            FROM ujian_jawabans j
            JOIN soals s ON s.id = j.soal_id
            WHERE j.ujian_id = ?
              AND j.nis = ?
              AND s.kategori = 'Uraian'
            "#,
        )
        .bind(ujian_id)
        .bind(nis)
        .fetch_one(db)
        .await;

        let total_uraian = match total_uraian {
            Ok(val) => val,
            Err(e) => {
                eprintln!(
                    "DEBUG finalize_submission: total_uraian query FAILED: {:?}",
                    e
                );
                0.0
            }
        };

        eprintln!("DEBUG finalize_submission: total_uraian = {}", total_uraian);

        // Total score = total_benar (PG yang benar) + total_uraian (essay yang sudah dinilai)
        let total_score = total_benar + total_uraian;
        eprintln!(
            "DEBUG finalize_submission: total_score = total_benar + total_uraian = {}",
            total_score
        );

        // Update peserta status and total_nilai
        let update_res = sqlx::query(
            r#"
            UPDATE ujian_pengerjaans
            SET status = 'submitted',
                submitted_at = NOW(),
                total_nilai = ?,
                updated_at = NOW()
            WHERE id = ?
            "#,
        )
        .bind(total_score)
        .bind(peserta_id)
        .execute(db)
        .await;
        if let Err(e) = update_res {
            eprintln!(
                "ERROR finalize_submission: update ujian_pengerjaans failed peserta_id={}, err={:?}",
                peserta_id, e
            );
        }

        eprintln!("DEBUG finalize_submission: akan insert/update hasil_nilais");

        // Insert/update hasil_nilais with totals
        let insert_res = sqlx::query(
            r#"
            INSERT INTO hasil_nilais
                (nis, ujian_id, mata_pelajaran_id, tahun, total_benar, total_salah, total_pg, total_uraian, total_nilai, created_at, updated_at)
            VALUES
                (?, ?, ?, ?, ?, ?, ?, ?, ?, NOW(), NOW())
            ON DUPLICATE KEY UPDATE
                total_benar = VALUES(total_benar),
                total_salah = VALUES(total_salah),
                total_uraian = VALUES(total_uraian),
                total_pg = VALUES(total_benar),
                total_nilai = VALUES(total_nilai),
                updated_at = NOW()
            "#,
        )
        .bind(nis)
        .bind(ujian_id)
        .bind(mata_pelajaran_id)
        .bind(&tahun)
        .bind(total_benar)
        .bind(total_salah)
        .bind(total_benar)
        .bind(total_uraian)
        .bind(total_score)
        .execute(db)
        .await;
        if let Err(e) = insert_res {
            eprintln!(
                "DEBUG finalize_submission: hasil_nilais insert failed: ujian_id={}, nis={}, mata_pelajaran_id={}, tahun={}, err={:?}",
                ujian_id, nis, mata_pelajaran_id, tahun, e
            );
        } else {
            eprintln!("DEBUG finalize_submission: hasil_nilais insert/update SUCCESS");
        }

        return total_score;
    }

    eprintln!("DEBUG finalize_submission: FALLBACK path - no ujian_meta");

    // Fallback: if no ujian meta, mark submitted and return 0
    let update_res = sqlx::query(
        r#"
        UPDATE ujian_pengerjaans
        SET status = 'submitted',
            submitted_at = NOW(),
            total_nilai = 0,
            updated_at = NOW()
        WHERE id = ?
        "#,
    )
    .bind(peserta_id)
    .execute(db)
    .await;
    if let Err(e) = update_res {
        eprintln!(
            "ERROR finalize_submission: fallback update ujian_pengerjaans failed peserta_id={}, err={:?}",
            peserta_id, e
        );
    }

    0.0
}

async fn simpan_jawaban_opsional(
    db: &MySqlPool,
    ujian_id: i64,
    nis: &str,
    soal_id: i64,
    pilihan: Option<String>,
    jawaban_uraian: Option<String>,
) {
    eprintln!(
        "DEBUG simpan_jawaban_opsional: ujian_id={}, nis={}, soal_id={}, pilihan={:?}, jawaban_uraian={:?}",
        ujian_id, nis, soal_id, pilihan, jawaban_uraian
    );

    let kunci_res = sqlx::query_as::<_, SoalKunciRow>(
        r#"
        SELECT s.kunci_jawaban, COALESCE(CAST(s.bobot_nilai AS DOUBLE), 1) as bobot_nilai, s.kategori
        FROM ujian_soals us
        JOIN soals s ON s.id = us.soal_id
        WHERE us.ujian_id = ? AND us.soal_id = ?
        LIMIT 1
        "#,
    )
    .bind(ujian_id)
    .bind(soal_id)
    .fetch_optional(db)
    .await;

    let kunci = match kunci_res {
        Ok(opt) => opt,
        Err(e) => {
            eprintln!(
                "ERROR simpan_jawaban_opsional: kunci query failed ujian_id={}, soal_id={}, err={:?}",
                ujian_id, soal_id, e
            );
            None
        }
    };
    let Some(k) = kunci else {
        eprintln!(
            "WARN simpan_jawaban_opsional: kunci not found for ujian_id={}, soal_id={}",
            ujian_id, soal_id
        );

        // extra diagnostics: check ujian_soals and soals existence and raw kunci_jawaban
        match sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM ujian_soals WHERE ujian_id = ? AND soal_id = ?",
        )
        .bind(ujian_id)
        .bind(soal_id)
        .fetch_one(db)
        .await
        {
            Ok(c) => eprintln!("DEBUG simpan_jawaban_opsional: ujian_soals.count = {}", c),
            Err(e) => eprintln!(
                "ERROR simpan_jawaban_opsional: ujian_soals count query failed: {:?}",
                e
            ),
        }

        match sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM soals WHERE id = ?")
            .bind(soal_id)
            .fetch_one(db)
            .await
        {
            Ok(c) => eprintln!("DEBUG simpan_jawaban_opsional: soals.count = {}", c),
            Err(e) => eprintln!(
                "ERROR simpan_jawaban_opsional: soals count query failed: {:?}",
                e
            ),
        }

        match sqlx::query_scalar::<_, Option<String>>(
            "SELECT kunci_jawaban FROM soals WHERE id = ?",
        )
        .bind(soal_id)
        .fetch_one(db)
        .await
        {
            Ok(kv) => eprintln!(
                "DEBUG simpan_jawaban_opsional: soals.kunci_jawaban = {:?}",
                kv
            ),
            Err(e) => eprintln!(
                "ERROR simpan_jawaban_opsional: soals kunci query failed: {:?}",
                e
            ),
        }

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

        let res = sqlx::query(
            r#"
            INSERT INTO ujian_jawabans
                (ujian_id, nis, soal_id, pilihan, jawaban_uraian, nilai_uraian, status_uraian, is_benar, bobot_nilai, created_at, updated_at)
                VALUES
                    (?, ?, ?, NULL, ?, 0, NULL, 0, 0, NOW(), NOW())
            ON DUPLICATE KEY UPDATE
                jawaban_uraian = VALUES(jawaban_uraian),
                nilai_uraian = 0,
                status_uraian = NULL,
                pilihan = NULL,
                is_benar = 0.00,
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
        if let Err(e) = res {
            eprintln!(
                "ERROR simpan_jawaban_opsional: failed insert uraian ujian_id={}, nis={}, soal_id={}, err={:?}",
                ujian_id, nis, soal_id, e
            );
        }
        return;
    }

    let Some(pilihan_raw) = pilihan else {
        return;
    };
    let kategori = k.kategori.as_deref().unwrap_or("Pilihan Ganda");

    if kategori == "Pilihan Ganda" {
        let pilihan = pilihan_raw.to_lowercase();
        if !matches!(pilihan.as_str(), "a" | "b" | "c" | "d") {
            return;
        }
        let is_benar = k
            .kunci_jawaban
            .as_deref()
            .map(|v| v.eq_ignore_ascii_case(&pilihan))
            .unwrap_or(false);
        let nilai = if is_benar { k.bobot_nilai } else { 0.0 };

        let res = sqlx::query(
            r#"
            INSERT INTO ujian_jawabans
                (ujian_id, nis, soal_id, pilihan, jawaban_uraian, nilai_uraian, status_uraian, is_benar, bobot_nilai, created_at, updated_at)
            VALUES
                (?, ?, ?, ?, NULL, 0, NULL, ?, ?, NOW(), NOW())
            ON DUPLICATE KEY UPDATE
                pilihan = VALUES(pilihan),
                jawaban_uraian = NULL,
                nilai_uraian = 0,
                status_uraian = NULL,
                is_benar = VALUES(is_benar),
                bobot_nilai = VALUES(bobot_nilai),
                updated_at = NOW()
            "#,
        )
        .bind(ujian_id)
        .bind(nis)
        .bind(soal_id)
        .bind(pilihan)
        .bind(if is_benar { 1 } else { 0 })
        .bind(nilai)
        .execute(db)
        .await;
        if let Err(e) = res {
            eprintln!(
                "ERROR simpan_jawaban_opsional: failed insert single ujian_id={}, nis={}, soal_id={}, err={:?}",
                ujian_id, nis, soal_id, e
            );
        }
        return;
    }

    if kategori == "Pilihan Ganda Kompleks" {
        // pilihan_raw expected like 'a' or 'a,c'
        let selected: Vec<String> = pilihan_raw
            .split(',')
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect();
        if selected.is_empty() {
            return;
        }
        // validate choices
        for s in &selected {
            if !matches!(s.as_str(), "a" | "b" | "c" | "d") {
                return;
            }
        }
        if selected.len() != 2 {
            return;
        }

        let correct: Vec<String> = k
            .kunci_jawaban
            .as_deref()
            .unwrap_or("")
            .split(',')
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect();

        let mut matches = 0usize;
        for s in &selected {
            if correct.iter().any(|c| c == s) {
                matches += 1;
            }
        }
        let matches_cap = std::cmp::min(matches, 2) as f64;
        let per_correct = k.bobot_nilai / 2.0;
        let nilai = matches_cap * per_correct;
        if correct.len() != 2 {
            return;
        }
        let is_benar = matches == correct.len() && correct.len() == 2;
        let pilihan_store = selected.join(",");

        let res = sqlx::query(
            r#"
            INSERT INTO ujian_jawabans
                (ujian_id, nis, soal_id, pilihan, jawaban_uraian, nilai_uraian, status_uraian, is_benar, bobot_nilai, created_at, updated_at)
            VALUES
                (?, ?, ?, ?, NULL, 0, NULL, ?, ?, NOW(), NOW())
            ON DUPLICATE KEY UPDATE
                pilihan = VALUES(pilihan),
                jawaban_uraian = NULL,
                nilai_uraian = 0,
                status_uraian = NULL,
                is_benar = VALUES(is_benar),
                bobot_nilai = VALUES(bobot_nilai),
                updated_at = NOW()
            "#,
        )
        .bind(ujian_id)
        .bind(nis)
        .bind(soal_id)
        .bind(pilihan_store)
        .bind(if is_benar { 1 } else { 0 })
        .bind(nilai)
        .execute(db)
        .await;
        if let Err(e) = res {
            eprintln!(
                "ERROR simpan_jawaban_opsional: failed insert kompleks ujian_id={}, nis={}, soal_id={}, err={:?}",
                ujian_id, nis, soal_id, e
            );
        }
        return;
    }

    if kategori == "Pilihan Ganda Kompleks MCMA" {
        let selected: Vec<String> = pilihan_raw
            .split(',')
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect();
        if selected.is_empty() || selected.len() > 4 {
            return;
        }
        let mut selected_unique = std::collections::HashSet::new();
        for s in &selected {
            if !matches!(s.as_str(), "a" | "b" | "c" | "d") || !selected_unique.insert(s.clone()) {
                return;
            }
        }

        let correct: Vec<String> = k
            .kunci_jawaban
            .as_deref()
            .unwrap_or("")
            .split(',')
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect();
        if correct.is_empty() || correct.len() > 4 {
            return;
        }
        let correct_set: std::collections::HashSet<&str> =
            correct.iter().map(|item| item.as_str()).collect();
        if correct_set.len() != correct.len()
            || !correct_set
                .iter()
                .all(|item| matches!(*item, "a" | "b" | "c" | "d"))
        {
            return;
        }

        let selected_set: std::collections::HashSet<&str> =
            selected.iter().map(|item| item.as_str()).collect();
        let matched_correct_count = correct
            .iter()
            .filter(|key| selected_set.contains(key.as_str()))
            .count();
        let total_correct = correct.len() as f64;
        let nilai = (matched_correct_count as f64 / total_correct) * k.bobot_nilai;
        let is_benar = selected_set.len() == correct_set.len() && selected_set == correct_set;
        let pilihan_store = selected.join(",");

        let res = sqlx::query(
            r#"
            INSERT INTO ujian_jawabans
                (ujian_id, nis, soal_id, pilihan, jawaban_uraian, nilai_uraian, status_uraian, is_benar, bobot_nilai, created_at, updated_at)
            VALUES
                (?, ?, ?, ?, NULL, 0, NULL, ?, ?, NOW(), NOW())
            ON DUPLICATE KEY UPDATE
                pilihan = VALUES(pilihan),
                jawaban_uraian = NULL,
                nilai_uraian = 0,
                status_uraian = NULL,
                is_benar = VALUES(is_benar),
                bobot_nilai = VALUES(bobot_nilai),
                updated_at = NOW()
            "#,
        )
        .bind(ujian_id)
        .bind(nis)
        .bind(soal_id)
        .bind(pilihan_store)
        .bind(if is_benar { 1 } else { 0 })
        .bind(nilai)
        .execute(db)
        .await;
        if let Err(e) = res {
            eprintln!(
                "ERROR simpan_jawaban_opsional: failed insert mcma ujian_id={}, nis={}, soal_id={}, err={:?}",
                ujian_id, nis, soal_id, e
            );
        }
        return;
    }

    if kategori == "Benar/Salah" {
        let pilihan_parts: Vec<String> = pilihan_raw
            .split(',')
            .map(|part| part.trim().to_lowercase())
            .filter(|part| !part.is_empty())
            .collect();
        if pilihan_parts.is_empty() {
            return;
        }

        let kunci_parts: Vec<String> = k
            .kunci_jawaban
            .as_deref()
            .unwrap_or("")
            .split(',')
            .map(|part| part.trim().to_lowercase())
            .filter(|part| !part.is_empty())
            .collect();

        if pilihan_parts.len() != kunci_parts.len() || kunci_parts.is_empty() {
            return;
        }

        let mut correct_count = 0usize;
        for (jawaban, kunci) in pilihan_parts.iter().zip(kunci_parts.iter()) {
            if !matches!(jawaban.as_str(), "benar" | "salah")
                || !matches!(kunci.as_str(), "benar" | "salah")
            {
                return;
            }
            if jawaban == kunci {
                correct_count += 1;
            }
        }

        let total = kunci_parts.len() as f64;
        let raw_ratio = correct_count as f64 / total;
        let ratio = if correct_count == kunci_parts.len() {
            1.0
        } else {
            (raw_ratio * 10.0).floor() / 10.0
        };
        let is_benar = correct_count == kunci_parts.len();
        let nilai = ratio * k.bobot_nilai;
        let pilihan_store = pilihan_parts.join(",");

        let res = sqlx::query(
            r#"
            INSERT INTO ujian_jawabans
                (ujian_id, nis, soal_id, pilihan, jawaban_uraian, nilai_uraian, status_uraian, is_benar, bobot_nilai, created_at, updated_at)
            VALUES
                (?, ?, ?, ?, NULL, 0, NULL, ?, ?, NOW(), NOW())
            ON DUPLICATE KEY UPDATE
                pilihan = VALUES(pilihan),
                jawaban_uraian = NULL,
                nilai_uraian = 0,
                status_uraian = NULL,
                is_benar = VALUES(is_benar),
                bobot_nilai = VALUES(bobot_nilai),
                updated_at = NOW()
            "#,
        )
        .bind(ujian_id)
        .bind(nis)
        .bind(soal_id)
        .bind(pilihan_store)
        .bind(if is_benar { 1 } else { 0 })
        .bind(nilai)
        .execute(db)
        .await;
        if let Err(e) = res {
            eprintln!(
                "ERROR simpan_jawaban_opsional: failed insert tf ujian_id={}, nis={}, soal_id={}, err={:?}",
                ujian_id, nis, soal_id, e
            );
        }
        return;
    }
}

fn build_true_false_statements(
    options: [Option<&str>; 3],
    jawaban_terpilih: Option<&str>,
) -> Vec<TrueFalseStatementView> {
    let answers: Vec<String> = jawaban_terpilih
        .unwrap_or("")
        .split(',')
        .map(|part| part.trim().to_lowercase())
        .filter(|part| !part.is_empty())
        .collect();

    let labels = ["1", "2", "3"];
    let mut answer_index = 0usize;
    let mut rows = Vec::new();

    for (label, option) in labels.iter().zip(options.iter()) {
        let Some(text) = option.and_then(|value| {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }) else {
            continue;
        };

        let jawaban = answers.get(answer_index).cloned();
        answer_index += 1;
        rows.push(TrueFalseStatementView {
            label: (*label).to_string(),
            text,
            jawaban,
        });
    }

    rows
}

async fn build_gate_data(
    db: &MySqlPool,
    nis: &str,
    message: Option<String>,
    error: Option<String>,
) -> UjianGateData {
    let tahun = data_tahun();
    let active_ujian = sqlx::query_as::<_, ActiveExamRow>(
        r#"
        SELECT
            u.id as ujian_id,
            u.title,
            u.description,
            u.waktu_menit,
            u.total_soal
        FROM ujians u
        JOIN ujian_tokens t ON t.ujian_id = u.id
        WHERE u.is_active = 1
          AND t.is_active = 1
          AND (t.expired_at IS NULL OR t.expired_at > NOW())
          AND u.tahun = ?
        ORDER BY t.created_at DESC
        LIMIT 1
        "#,
    )
    .bind(&tahun)
    .fetch_optional(db)
    .await
    .unwrap_or(None);

    let active_ujian = if let Some(exam) = active_ujian {
        let peserta = sqlx::query_as::<_, PesertaStatusRow>(
            "SELECT id, status FROM ujian_pengerjaans WHERE ujian_id = ? AND nis = ? LIMIT 1",
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
