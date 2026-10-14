use std::collections::HashMap;

use axum::{
    Extension, Form,
    extract::{Path, Query},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
};
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, MySql, MySqlPool, Transaction};

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
    timer_server_now_ms: i64,
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
    last_nomor: i32,
    status: String,
    timer_deadline_ms: i64,
    timer_server_now_ms: i64,
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
            p.last_nomor,
            p.status,
            CAST(UNIX_TIMESTAMP(TIMESTAMPADD(MINUTE, u.waktu_menit, p.started_at)) * 1000 AS SIGNED) AS timer_deadline_ms,
            CAST(UNIX_TIMESTAMP(NOW(3)) * 1000 AS SIGNED) AS timer_server_now_ms
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

    if deadline_reached(header.timer_server_now_ms, header.timer_deadline_ms) {
        eprintln!(
            "DEBUG ujian_session_page: waktu habis, auto finalize - peserta_id={}, ujian_id={}, nis={}",
            peserta_id, header.ujian_id, nis
        );
        if let Err(error) = finalize_submission(&db, header.ujian_id, &nis, peserta_id).await {
            eprintln!("ERROR auto finalize expired session {peserta_id}: {error:?}");
        }
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
        timer_deadline_ms: header.timer_deadline_ms,
        timer_server_now_ms: header.timer_server_now_ms,
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

    let mut tx = match db.begin().await {
        Ok(tx) => tx,
        Err(error) => {
            eprintln!("ERROR begin save-answer transaction: {error:?}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    let peserta = match lock_owned_session(&mut tx, peserta_id, &nis).await {
        Ok(peserta) => peserta,
        Err(error) => {
            eprintln!("ERROR lock save-answer session: {error:?}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let Some(peserta) = peserta else {
        return Redirect::to("/siswa/ujian?message=Sesi+ujian+tidak+ditemukan").into_response();
    };

    if peserta.status == "submitted" {
        return Redirect::to("/siswa/ujian?message=Ujian+sudah+disubmit+dan+tidak+dapat+diubah")
            .into_response();
    }

    if peserta.is_expired() {
        drop(tx);
        if let Err(error) = finalize_submission(&db, peserta.ujian_id, &nis, peserta_id).await {
            eprintln!("ERROR finalize expired save-answer request: {error:?}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
        return Redirect::to("/siswa/ujian?message=Waktu+ujian+habis.+Jawaban+otomatis+disubmit")
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
        &mut tx,
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
        sqlx::query("UPDATE ujian_pengerjaans SET last_nomor = ?, updated_at = NOW() WHERE id = ? AND status = 'started'")
            .bind(nomor_tujuan)
            .bind(peserta_id)
            .execute(&mut *tx)
            .await;

    if let Err(error) = tx.commit().await {
        eprintln!("ERROR commit save-answer transaction: {error:?}");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

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
    timer_deadline_ms: i64,
    timer_server_now_ms: i64,
}

impl PesertaOwnerRow {
    fn is_expired(&self) -> bool {
        deadline_reached(self.timer_server_now_ms, self.timer_deadline_ms)
    }
}

fn deadline_reached(server_now_ms: i64, deadline_ms: i64) -> bool {
    server_now_ms >= deadline_ms
}

async fn lock_owned_session(
    tx: &mut Transaction<'_, MySql>,
    peserta_id: i64,
    nis: &str,
) -> Result<Option<PesertaOwnerRow>, sqlx::Error> {
    sqlx::query_as::<_, PesertaOwnerRow>(
        r#"
        SELECT
            p.ujian_id,
            p.status,
            CAST(UNIX_TIMESTAMP(TIMESTAMPADD(MINUTE, u.waktu_menit, p.started_at)) * 1000 AS SIGNED) AS timer_deadline_ms,
            CAST(UNIX_TIMESTAMP(NOW(3)) * 1000 AS SIGNED) AS timer_server_now_ms
        FROM ujian_pengerjaans p
        JOIN ujians u ON u.id = p.ujian_id
        WHERE p.id = ? AND p.nis = ?
        LIMIT 1
        FOR UPDATE
        "#,
    )
    .bind(peserta_id)
    .bind(nis)
    .fetch_optional(&mut **tx)
    .await
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

    let mut tx = match db.begin().await {
        Ok(tx) => tx,
        Err(error) => {
            eprintln!("ERROR begin submit transaction: {error:?}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    let peserta = match lock_owned_session(&mut tx, peserta_id, &nis).await {
        Ok(peserta) => peserta,
        Err(error) => {
            eprintln!("ERROR lock submit session: {error:?}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

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

    if !peserta.is_expired()
        && let Some(soal_id) = form.soal_id
    {
        eprintln!(
            "DEBUG ujian_submit: akan menyimpan jawaban soal_id={}",
            soal_id
        );
        simpan_jawaban_opsional(
            &mut tx,
            peserta.ujian_id,
            &nis,
            soal_id,
            form.pilihan.clone(),
            form.jawaban_uraian.clone(),
        )
        .await;
    }

    if let Err(error) = sqlx::query(
        r#"
        UPDATE ujian_pengerjaans
        SET status = 'submitted', submitted_at = COALESCE(submitted_at, NOW()), updated_at = NOW()
        WHERE id = ? AND status = 'started'
        "#,
    )
    .bind(peserta_id)
    .execute(&mut *tx)
    .await
    {
        eprintln!("ERROR mark session submitted: {error:?}");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    if let Err(error) = tx.commit().await {
        eprintln!("ERROR commit submit transaction: {error:?}");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    eprintln!(
        "DEBUG ujian_submit: akan memanggil finalize_submission untuk ujian_id={}, nis={}",
        peserta.ujian_id, nis
    );
    let total_nilai = match finalize_submission(&db, peserta.ujian_id, &nis, peserta_id).await {
        Ok(total_nilai) => total_nilai,
        Err(error) => {
            eprintln!("ERROR finalize submitted session: {error:?}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    eprintln!(
        "DEBUG ujian_submit: finalize_submission selesai, total_nilai={}",
        total_nilai
    );

    Redirect::to(
        "/siswa/ujian?message=Ujian+berhasil+disubmit.+Nilai+akhir+akan+diproses+di+dashboard+guru/admin",
    )
    .into_response()
}

async fn finalize_submission(
    db: &MySqlPool,
    ujian_id: i64,
    nis: &str,
    peserta_id: i64,
) -> Result<f64, sqlx::Error> {
    let mut tx = db.begin().await?;
    let session: Option<(String, Option<f64>)> = sqlx::query_as(
        "SELECT status, CAST(total_nilai AS DOUBLE) FROM ujian_pengerjaans WHERE id = ? AND ujian_id = ? AND nis = ? FOR UPDATE",
    )
    .bind(peserta_id)
    .bind(ujian_id)
    .bind(nis)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((status, stored_total)) = session else {
        return Err(sqlx::Error::RowNotFound);
    };
    if status == "submitted"
        && let Some(total) = stored_total
    {
        tx.commit().await?;
        return Ok(total);
    }

    let (mata_pelajaran_id, tahun): (i64, String) = sqlx::query_as(
        "SELECT CAST(mata_pelajaran_id AS SIGNED), CAST(tahun AS CHAR) FROM ujians WHERE id = ? LIMIT 1",
    )
    .bind(ujian_id)
    .fetch_one(&mut *tx)
    .await?;

    let total_benar: f64 = sqlx::query_scalar(
        r#"
        SELECT COALESCE(SUM(CAST(COALESCE(bobot_nilai, 0) AS DOUBLE)), 0)
        FROM ujian_jawabans
        WHERE ujian_id = ? AND nis = ?
        "#,
    )
    .bind(ujian_id)
    .bind(nis)
    .fetch_one(&mut *tx)
    .await?;
    let total_possible_non_uraian: f64 = sqlx::query_scalar(
        r#"
        SELECT COALESCE(SUM(CAST(COALESCE(s.bobot_nilai, 0) AS DOUBLE)), 0)
        FROM ujian_soals us
        JOIN soals s ON s.id = us.soal_id
        WHERE us.ujian_id = ? AND s.kategori <> 'Uraian'
        "#,
    )
    .bind(ujian_id)
    .fetch_one(&mut *tx)
    .await?;
    let total_salah = (total_possible_non_uraian - total_benar).max(0.0);
    let total_uraian: f64 = sqlx::query_scalar(
        r#"
        SELECT COALESCE(SUM(CAST(COALESCE(j.nilai_uraian, 0) AS DOUBLE)), 0)
        FROM ujian_jawabans j
        JOIN soals s ON s.id = j.soal_id
        WHERE j.ujian_id = ? AND j.nis = ? AND s.kategori = 'Uraian'
        "#,
    )
    .bind(ujian_id)
    .bind(nis)
    .fetch_one(&mut *tx)
    .await?;
    let total_score = total_benar + total_uraian;

    sqlx::query(
        r#"
        INSERT INTO hasil_nilais
            (nis, ujian_id, mata_pelajaran_id, tahun, total_benar, total_salah, total_pg, total_uraian, total_nilai, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, NOW(), NOW())
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
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        UPDATE ujian_pengerjaans
        SET status = 'submitted',
            submitted_at = COALESCE(submitted_at, NOW()),
            total_nilai = ?,
            updated_at = NOW()
        WHERE id = ?
        "#,
    )
    .bind(total_score)
    .bind(peserta_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    Ok(total_score)
}

#[derive(FromRow)]
struct ExpiredSessionRow {
    id: i64,
    ujian_id: i64,
    nis: String,
}

pub async fn finalize_expired_sessions(db: &MySqlPool) -> Result<u64, sqlx::Error> {
    let sessions = sqlx::query_as::<_, ExpiredSessionRow>(
        r#"
        SELECT p.id, p.ujian_id, p.nis
        FROM ujian_pengerjaans p
        JOIN ujians u ON u.id = p.ujian_id
        WHERE (p.status = 'started' AND NOW() >= TIMESTAMPADD(MINUTE, u.waktu_menit, p.started_at))
           OR (p.status = 'submitted' AND p.total_nilai IS NULL)
        ORDER BY p.id
        LIMIT 100
        "#,
    )
    .fetch_all(db)
    .await?;

    let mut finalized = 0;
    for session in sessions {
        match finalize_submission(db, session.ujian_id, &session.nis, session.id).await {
            Ok(_) => finalized += 1,
            Err(error) => eprintln!(
                "ERROR periodic finalize session id={} ujian_id={}: {error:?}",
                session.id, session.ujian_id
            ),
        }
    }

    Ok(finalized)
}

fn parse_option_list(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|part| part.trim().to_lowercase())
        .filter(|part| !part.is_empty())
        .collect()
}

fn proportional_score(correct_count: usize, total_correct: usize, bobot_nilai: f64) -> f64 {
    if total_correct == 0 || bobot_nilai <= 0.0 {
        return 0.0;
    }

    (correct_count as f64 / total_correct as f64) * bobot_nilai
}

async fn simpan_jawaban_opsional(
    tx: &mut Transaction<'_, MySql>,
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
    .fetch_optional(&mut **tx)
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
        .fetch_one(&mut **tx)
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
            .fetch_one(&mut **tx)
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
        .fetch_one(&mut **tx)
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
        .execute(&mut **tx)
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
        .execute(&mut **tx)
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
        let selected = parse_option_list(&pilihan_raw);
        if selected.is_empty() {
            return;
        }
        let mut selected_unique = std::collections::HashSet::new();
        for pilihan in &selected {
            if !matches!(pilihan.as_str(), "a" | "b" | "c" | "d")
                || !selected_unique.insert(pilihan.clone())
            {
                return;
            }
        }

        let correct = parse_option_list(k.kunci_jawaban.as_deref().unwrap_or(""));
        if correct.is_empty() || correct.len() > 4 {
            return;
        }

        let mut correct_unique = std::collections::HashSet::new();
        for kunci in &correct {
            if !matches!(kunci.as_str(), "a" | "b" | "c" | "d")
                || !correct_unique.insert(kunci.clone())
            {
                return;
            }
        }

        if selected.len() != correct.len() {
            return;
        }

        let matched_correct_count = correct
            .iter()
            .filter(|kunci| selected_unique.contains(kunci.as_str()))
            .count();
        let nilai = proportional_score(matched_correct_count, correct.len(), k.bobot_nilai);
        let is_benar = matched_correct_count == correct.len();
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
        .execute(&mut **tx)
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
        let selected = parse_option_list(&pilihan_raw);
        if selected.is_empty() || selected.len() > 4 {
            return;
        }
        let mut selected_unique = std::collections::HashSet::new();
        for s in &selected {
            if !matches!(s.as_str(), "a" | "b" | "c" | "d") || !selected_unique.insert(s.clone()) {
                return;
            }
        }

        let correct = parse_option_list(k.kunci_jawaban.as_deref().unwrap_or(""));
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
        let nilai = proportional_score(matched_correct_count, correct.len(), k.bobot_nilai);
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
        .execute(&mut **tx)
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
        let pilihan_parts = parse_option_list(&pilihan_raw);
        if pilihan_parts.is_empty() {
            return;
        }

        let kunci_parts = parse_option_list(k.kunci_jawaban.as_deref().unwrap_or(""));

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

        let is_benar = correct_count == kunci_parts.len();
        let nilai = proportional_score(correct_count, kunci_parts.len(), k.bobot_nilai);
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
        .execute(&mut **tx)
        .await;
        if let Err(e) = res {
            eprintln!(
                "ERROR simpan_jawaban_opsional: failed insert tf ujian_id={}, nis={}, soal_id={}, err={:?}",
                ujian_id, nis, soal_id, e
            );
        }
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

#[cfg(test)]
mod tests {
    use super::{deadline_reached, proportional_score};

    #[test]
    fn deadline_is_enforced_before_at_and_after_boundary() {
        assert!(!deadline_reached(999, 1_000));
        assert!(deadline_reached(1_000, 1_000));
        assert!(deadline_reached(1_001, 1_000));
    }

    #[test]
    fn objective_scoring_formula_is_unchanged() {
        assert_eq!(proportional_score(2, 4, 10.0), 5.0);
        assert_eq!(proportional_score(0, 4, 10.0), 0.0);
    }
}
