use axum::{
    Extension,
    extract::{Form, FromRequestParts, Query},
    http::{HeaderMap, StatusCode, request::Parts},
    response::{Html, IntoResponse, Redirect},
};
use chrono::Local;
use serde::{Deserialize, Serialize};
use sqlx::MySqlPool;
use tera::Context;

use crate::utils::{
    list_kelas::list_kelas, page_context::PageContext, render::render, semester::data_semester,
    tahun::data_tahun,
};

#[derive(Serialize)]
struct AbsensiData {
    tahun: String,
    semester: i32,
    kelas_id: i32,
    tanggal: String,
    jam: String,
    require_kelas: bool,
    list_kelas: Vec<crate::utils::list_kelas::ListKelas>,
}

#[derive(Deserialize, Clone)]
pub struct AbsensiFilter {
    pub tahun: Option<String>,
    pub semester: Option<i32>,
    #[serde(default, deserialize_with = "empty_string_as_none_i32")]
    pub kelas_id: Option<i32>,
    pub tanggal: Option<String>,
    #[serde(default, deserialize_with = "empty_string_as_none_string")]
    pub jam: Option<String>,
}

#[derive(Serialize)]
struct AbsensiRow {
    nis: Option<String>,
    nama: Option<String>,
    kelas: Option<String>,
    kehadiran_id: Option<i32>,
    absensi_id: Option<i64>,
    guru_nama: Option<String>,
}

pub struct Htmx(pub bool);

impl<S> FromRequestParts<S> for Htmx
where
    S: Send + Sync,
{
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        let is_htmx = parts
            .headers
            .get("HX-Request")
            .and_then(|v| v.to_str().ok())
            .map(|v| v == "true")
            .unwrap_or(false);

        Ok(Htmx(is_htmx))
    }
}

pub async fn absensi_kelas(ctx: PageContext, Extension(db): Extension<MySqlPool>) -> Html<String> {
    let data = AbsensiData {
        tahun: data_tahun(),
        semester: data_semester(),
        kelas_id: 0,
        tanggal: Local::now().format("%Y-%m-%d").to_string(),
        jam: "".to_string(),
        require_kelas: true,
        list_kelas: list_kelas(&db).await.unwrap_or_default(),
    };

    render(&ctx, "guru/absensi_kelas.html", "Absensi Kelas", data)
}

pub async fn absensi_kelas_table(
    ctx: PageContext,
    Htmx(is_htmx): Htmx,
    Query(filter): Query<AbsensiFilter>,
    Extension(db): Extension<MySqlPool>,
) -> axum::response::Response {
    let (tahun, semester, kelas_id, tanggal, jam) = normalize_filter(filter);

    if !is_htmx {
        let url = format!(
            "/absensi-kelas?tahun={}&semester={}&kelas_id={}&tanggal={}&jam={}",
            tahun, semester, kelas_id, tanggal, jam
        );
        return Redirect::to(&url).into_response();
    }

    let rendered = render_table(&ctx, &db, &tahun, semester, kelas_id, &tanggal, &jam).await;
    Html(rendered).into_response()
}

pub async fn absensi_kelas_mark_all(
    ctx: PageContext,
    Htmx(is_htmx): Htmx,
    Extension(db): Extension<MySqlPool>,
    Form(filter): Form<AbsensiFilter>,
) -> axum::response::Response {
    let (tahun, semester, kelas_id, tanggal, jam) = normalize_filter(filter);

    if !is_htmx {
        let url = format!(
            "/absensi-kelas?tahun={}&semester={}&kelas_id={}&tanggal={}&jam={}",
            tahun, semester, kelas_id, tanggal, jam
        );
        return Redirect::to(&url).into_response();
    }

    if kelas_id == 0 || jam.is_empty() {
        let rendered = render_table(&ctx, &db, &tahun, semester, kelas_id, &tanggal, &jam).await;
        return Html(rendered).into_response();
    }

    let _ = sqlx::query!(
        r#"
        INSERT INTO absensis (tanggal, tahun, semester, jam, kelas_id, nis, kehadiran_id, user_id, created_at, updated_at)
        SELECT ?, ?, ?, ?, s.kelas_id, s.nis, 1, ?, NOW(), NOW()
        FROM siswas s
        WHERE s.tahun = ?
          AND (? = 0 OR s.kelas_id = ?)
          AND NOT EXISTS (
            SELECT 1
            FROM absensis a
            WHERE a.tahun = ?
              AND a.semester = ?
              AND a.kelas_id = s.kelas_id
              AND a.tanggal = ?
              AND a.jam = ?
          )
        "#,
        tanggal,
        tahun,
        semester,
        jam,
        ctx.user.id as i64,
        tahun,
        kelas_id,
        kelas_id,
        tahun,
        semester,
        tanggal,
        jam
    )
    .execute(&db)
    .await;

    let rendered = render_table(&ctx, &db, &tahun, semester, kelas_id, &tanggal, &jam).await;
    let mut headers = HeaderMap::new();
    headers.insert(
        "HX-Trigger",
        r#"{"flash":{"message":"Semua siswa sudah dihadirkan","type":"success"}}"#
            .parse()
            .unwrap(),
    );

    (headers, Html(rendered)).into_response()
}

#[derive(Deserialize)]
pub struct AbsensiUpdateForm {
    pub absensi_id: Option<i64>,
    pub kehadiran_id: i64,
    pub nis: String,
    pub tahun: String,
    pub semester: i32,
    pub kelas_id: i32,
    pub tanggal: String,
    pub jam: String,
}

pub async fn absensi_kelas_update(
    Htmx(is_htmx): Htmx,
    Extension(db): Extension<MySqlPool>,
    Form(payload): Form<AbsensiUpdateForm>,
) -> axum::response::Response {
    if !is_htmx {
        return StatusCode::BAD_REQUEST.into_response();
    }

    if let Some(absensi_id) = payload.absensi_id.filter(|id| *id > 0) {
        let _ = sqlx::query!(
            r#"
            UPDATE absensis
            SET kehadiran_id = ?, updated_at = NOW()
            WHERE id = ?
            "#,
            payload.kehadiran_id,
            absensi_id
        )
        .execute(&db)
        .await;
    } else {
        let _ = sqlx::query!(
            r#"
            INSERT INTO absensis (tanggal, tahun, semester, jam, kelas_id, nis, kehadiran_id, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, NOW(), NOW())
            "#,
            payload.tanggal,
            payload.tahun,
            payload.semester,
            payload.jam,
            payload.kelas_id,
            payload.nis,
            payload.kehadiran_id
        )
        .execute(&db)
        .await;
    }

    let mut headers = HeaderMap::new();
    headers.insert(
        "HX-Trigger",
        r#"{"flash":{"message":"Kehadiran siswa diperbarui","type":"info"}}"#
            .parse()
            .unwrap(),
    );

    (headers, StatusCode::OK).into_response()
}

fn normalize_filter(filter: AbsensiFilter) -> (String, i32, i32, String, String) {
    let tahun = filter.tahun.unwrap_or_else(data_tahun);
    let semester = filter.semester.unwrap_or_else(data_semester);
    let kelas_id = filter.kelas_id.unwrap_or(0);
    let tanggal = filter
        .tanggal
        .unwrap_or_else(|| Local::now().format("%Y-%m-%d").to_string());
    let jam = filter.jam.unwrap_or_default();

    (tahun, semester, kelas_id, tanggal, jam)
}

async fn render_table(
    ctx: &PageContext,
    db: &MySqlPool,
    tahun: &str,
    semester: i32,
    kelas_id: i32,
    tanggal: &str,
    jam: &str,
) -> String {
    if kelas_id == 0 {
        let mut tera_ctx = Context::new();
        tera_ctx.insert("rows", &Vec::<AbsensiRow>::new());
        tera_ctx.insert("show_empty", &true);
        tera_ctx.insert("empty_message", "Pilih kelas terlebih dahulu");
        return ctx
            .tera
            .render("guru/_absensi_table.html", &tera_ctx)
            .unwrap();
    }
    if jam.is_empty() {
        let mut tera_ctx = Context::new();
        tera_ctx.insert("rows", &Vec::<AbsensiRow>::new());
        tera_ctx.insert("show_empty", &true);
        tera_ctx.insert("empty_message", "Pilih jam terlebih dahulu");
        return ctx
            .tera
            .render("guru/_absensi_table.html", &tera_ctx)
            .unwrap();
    }

    let rows: Vec<AbsensiRow> = sqlx::query_as!(
        AbsensiRow,
        r#"
        SELECT 
            s.nis,
            u.name AS nama,
            k.nama AS kelas,
            CAST(a.kehadiran_id AS SIGNED) AS kehadiran_id,
            CAST(a.id AS SIGNED) AS absensi_id,
            g.name AS guru_nama
        FROM siswas s
        JOIN users u ON u.nis = s.nis
        JOIN kelas k ON k.id = s.kelas_id
        LEFT JOIN absensis a
            ON a.nis = s.nis
           AND a.tahun = ?
           AND a.semester = ?
           AND a.kelas_id = s.kelas_id
           AND a.tanggal = ?
           AND a.jam = ?
        LEFT JOIN users g ON g.id = a.user_id
        WHERE s.tahun = ?
          AND s.kelas_id = ?
        ORDER BY u.name
        "#,
        tahun,
        semester,
        tanggal,
        jam,
        tahun,
        kelas_id
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();

    let mut tera_ctx = Context::new();
    tera_ctx.insert("rows", &rows);
    tera_ctx.insert("show_empty", &rows.is_empty());
    tera_ctx.insert("filter_tahun", tahun);
    tera_ctx.insert("filter_semester", &semester);
    tera_ctx.insert("filter_kelas_id", &kelas_id);
    tera_ctx.insert("filter_tanggal", tanggal);
    tera_ctx.insert("filter_jam", jam);
    if rows.is_empty() {
        tera_ctx.insert("empty_message", "Tidak ada data siswa");
    }

    ctx.tera
        .render("guru/_absensi_table.html", &tera_ctx)
        .unwrap()
}

fn empty_string_as_none_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt = Option::<String>::deserialize(deserializer)?;
    Ok(opt.and_then(|s| {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    }))
}

fn empty_string_as_none_i32<'de, D>(deserializer: D) -> Result<Option<i32>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt = Option::<String>::deserialize(deserializer)?;
    Ok(opt.and_then(|s| {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            None
        } else {
            trimmed.parse::<i32>().ok()
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::{
        AbsensiFilter, AbsensiUpdateForm, Htmx, absensi_kelas, absensi_kelas_update,
        empty_string_as_none_i32, empty_string_as_none_string, normalize_filter,
    };
    use crate::{
        models::{auth_user::AuthUser, role::Role},
        utils::page_context::PageContext,
    };
    use axum::http::Uri;
    use axum::response::IntoResponse;
    use axum::{Extension, Form};
    use chrono::Local;
    use serial_test::serial;

    #[test]
    fn normalize_filter_sets_defaults() {
        let filter = AbsensiFilter {
            tahun: None,
            semester: None,
            kelas_id: None,
            tanggal: None,
            jam: None,
        };
        let (tahun, semester, kelas_id, tanggal, jam) = normalize_filter(filter);
        assert!(!tahun.is_empty());
        assert!(semester == 1 || semester == 2);
        assert_eq!(kelas_id, 0);
        assert!(!tanggal.is_empty());
        assert!(jam.is_empty());
    }

    #[test]
    fn empty_string_deserializers_handle_blank_values() {
        use serde::de::IntoDeserializer;

        let de = serde_json::json!("").into_deserializer();
        let parsed = empty_string_as_none_string(de).unwrap();
        assert_eq!(parsed, None);

        let de = serde_json::json!(" ").into_deserializer();
        let parsed = empty_string_as_none_i32(de).unwrap();
        assert_eq!(parsed, None);
    }

    #[tokio::test]
    #[serial]
    async fn absensi_update_inserts_when_missing_id() {
        let Some(test_db) = crate::test_support::TestDb::try_new().await else {
            return;
        };
        let seed = crate::test_support::seed_base_data(&test_db.pool).await;

        let payload = AbsensiUpdateForm {
            absensi_id: None,
            kehadiran_id: 1,
            nis: seed.nis.clone(),
            tahun: seed.tahun.clone(),
            semester: 1,
            kelas_id: seed.kelas_id as i32,
            tanggal: Local::now().format("%Y-%m-%d").to_string(),
            jam: "1".into(),
        };

        let response =
            absensi_kelas_update(Htmx(true), Extension(test_db.pool.clone()), Form(payload))
                .await
                .into_response();

        assert_eq!(response.status(), axum::http::StatusCode::OK);

        let count: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM absensis")
            .fetch_one(&test_db.pool)
            .await
            .unwrap_or(0);
        assert_eq!(count, 1);

        test_db.teardown().await;
    }

    #[tokio::test]
    #[serial]
    async fn absensi_kelas_page_renders() {
        let Some(test_db) = crate::test_support::TestDb::try_new().await else {
            return;
        };
        crate::test_support::seed_base_data(&test_db.pool).await;

        let response = absensi_kelas(base_ctx(), Extension(test_db.pool.clone()))
            .await
            .into_response();

        assert_eq!(response.status(), axum::http::StatusCode::OK);
        test_db.teardown().await;
    }

    fn base_ctx() -> PageContext {
        PageContext {
            user: AuthUser {
                id: 1,
                roles: vec![Role::Guru],
                name: "Guru".into(),
                foto: None,
            },
            tera: crate::test_support::build_test_tera(),
            uri: Uri::from_static("/absensi-kelas"),
        }
    }

    #[tokio::test]
    async fn render_table_short_circuits_without_kelas_or_jam() {
        let ctx = base_ctx();
        let html = super::render_table(
            &ctx,
            &sqlx::MySqlPool::connect_lazy("mysql://user:pass@localhost/db").unwrap(),
            "2024 / 2025",
            1,
            0,
            "2024-01-01",
            "",
        )
        .await;
        assert!(
            html.contains("Pilih kelas terlebih dahulu")
                || html.contains("Pilih jam terlebih dahulu")
        );
    }
}
