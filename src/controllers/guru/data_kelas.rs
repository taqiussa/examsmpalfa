use axum::{
    Extension, Form,
    extract::Path,
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect},
};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, MySql, MySqlPool, Transaction};
use std::collections::HashMap;

use crate::{
    controllers::guru::absensi_kelas::Htmx,
    utils::{page_context::PageContext, render::render},
};

#[derive(Debug, Serialize, FromRow)]
struct KelasRow {
    id: i64,
    nama: String,
    tingkat: String,
}

#[derive(Serialize)]
struct KelasListData {
    rows: Vec<KelasRow>,
}

#[derive(Serialize)]
struct KelasFormData {
    nama: String,
    tingkat: String,
    errors: HashMap<String, String>,
    action: String,
    page_title: String,
    submit_label: String,
}

#[derive(Deserialize)]
pub struct KelasForm {
    nama: String,
    tingkat: String,
}

pub async fn data_kelas_index(
    ctx: PageContext,
    Extension(db): Extension<MySqlPool>,
) -> Html<String> {
    render(
        &ctx,
        "guru/data_kelas/index.html",
        "Data Kelas",
        KelasListData {
            rows: fetch_kelas(&db).await,
        },
    )
}

pub async fn data_kelas_create(ctx: PageContext) -> Html<String> {
    render_form_page(
        &ctx,
        create_form_data(String::new(), String::new(), HashMap::new()),
    )
}

pub async fn data_kelas_edit(
    ctx: PageContext,
    Path(id): Path<i64>,
    Extension(db): Extension<MySqlPool>,
) -> axum::response::Response {
    match find_kelas(&db, id).await {
        Ok(Some(row)) => render_form_page(
            &ctx,
            edit_form_data(row.id, row.nama, row.tingkat, HashMap::new()),
        )
        .into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, "Data kelas tidak ditemukan.").into_response(),
        Err(error) => {
            eprintln!("ERROR data_kelas_edit: {error:?}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Gagal mengambil data kelas.",
            )
                .into_response()
        }
    }
}

pub async fn data_kelas_store(
    ctx: PageContext,
    Htmx(is_htmx): Htmx,
    Extension(db): Extension<MySqlPool>,
    Form(form): Form<KelasForm>,
) -> axum::response::Response {
    let (nama, tingkat, mut errors) = normalize_and_validate(form);

    if errors.is_empty() && nama_exists(&db, &nama, None).await {
        errors.insert("nama".into(), "Nama kelas sudah digunakan.".into());
    }

    if !errors.is_empty() {
        return form_error_response(&ctx, is_htmx, create_form_data(nama, tingkat, errors));
    }

    match sqlx::query(
        "INSERT INTO kelas (nama, tingkat, created_at, updated_at) VALUES (?, ?, NOW(), NOW())",
    )
    .bind(&nama)
    .bind(&tingkat)
    .execute(&db)
    .await
    {
        Ok(_) => success_response(is_htmx, "Data kelas berhasil ditambahkan."),
        Err(error) => {
            eprintln!("ERROR data_kelas_store: {error:?}");
            errors.insert(
                "nama".into(),
                "Data kelas gagal disimpan. Silakan coba lagi.".into(),
            );
            form_error_response(&ctx, is_htmx, create_form_data(nama, tingkat, errors))
        }
    }
}

pub async fn data_kelas_update(
    ctx: PageContext,
    Htmx(is_htmx): Htmx,
    Path(id): Path<i64>,
    Extension(db): Extension<MySqlPool>,
    Form(form): Form<KelasForm>,
) -> axum::response::Response {
    if !kelas_exists(&db, id).await {
        return (StatusCode::NOT_FOUND, "Data kelas tidak ditemukan.").into_response();
    }

    let (nama, tingkat, mut errors) = normalize_and_validate(form);
    if errors.is_empty() && nama_exists(&db, &nama, Some(id)).await {
        errors.insert("nama".into(), "Nama kelas sudah digunakan.".into());
    }

    if !errors.is_empty() {
        return form_error_response(&ctx, is_htmx, edit_form_data(id, nama, tingkat, errors));
    }

    match sqlx::query("UPDATE kelas SET nama = ?, tingkat = ?, updated_at = NOW() WHERE id = ?")
        .bind(&nama)
        .bind(&tingkat)
        .bind(id)
        .execute(&db)
        .await
    {
        Ok(_) => success_response(is_htmx, "Data kelas berhasil diperbarui."),
        Err(error) => {
            eprintln!("ERROR data_kelas_update: {error:?}");
            errors.insert(
                "nama".into(),
                "Data kelas gagal diperbarui. Silakan coba lagi.".into(),
            );
            form_error_response(&ctx, is_htmx, edit_form_data(id, nama, tingkat, errors))
        }
    }
}

pub async fn data_kelas_destroy(
    Htmx(is_htmx): Htmx,
    Path(id): Path<i64>,
    Extension(db): Extension<MySqlPool>,
) -> axum::response::Response {
    match delete_kelas_if_unused(&db, id).await {
        Ok(DeleteResult::Deleted) => success_response(is_htmx, "Data kelas berhasil dihapus."),
        Ok(DeleteResult::NotFound) => {
            (StatusCode::NOT_FOUND, "Data kelas tidak ditemukan.").into_response()
        }
        Ok(DeleteResult::InUse) => error_response(
            is_htmx,
            "Data kelas tidak dapat dihapus karena masih digunakan oleh data lain.",
        ),
        Err(error) => {
            eprintln!("ERROR data_kelas_destroy: {error:?}");
            error_response(is_htmx, "Data kelas gagal dihapus. Silakan coba lagi.")
        }
    }
}

enum DeleteResult {
    Deleted,
    NotFound,
    InUse,
}

async fn delete_kelas_if_unused(db: &MySqlPool, id: i64) -> Result<DeleteResult, sqlx::Error> {
    let mut tx = db.begin().await?;
    let exists = sqlx::query_scalar::<_, i64>(
        "SELECT CAST(id AS SIGNED) FROM kelas WHERE id = ? LIMIT 1 FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?
    .is_some();

    if !exists {
        tx.rollback().await?;
        return Ok(DeleteResult::NotFound);
    }

    if kelas_has_references(&mut tx, id).await? {
        tx.rollback().await?;
        return Ok(DeleteResult::InUse);
    }

    sqlx::query("DELETE FROM kelas WHERE id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(DeleteResult::Deleted)
}

async fn kelas_has_references(
    tx: &mut Transaction<'_, MySql>,
    id: i64,
) -> Result<bool, sqlx::Error> {
    let tables = sqlx::query_scalar::<_, String>(
        r#"
        SELECT TABLE_NAME
        FROM information_schema.COLUMNS
        WHERE TABLE_SCHEMA = DATABASE()
          AND COLUMN_NAME = 'kelas_id'
        ORDER BY TABLE_NAME
        "#,
    )
    .fetch_all(&mut **tx)
    .await?;

    for table in tables {
        if !table
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
        {
            continue;
        }
        let query = format!("SELECT COUNT(*) FROM `{table}` WHERE kelas_id = ?");
        let count = sqlx::query_scalar::<_, i64>(&query)
            .bind(id)
            .fetch_one(&mut **tx)
            .await?;
        if count > 0 {
            return Ok(true);
        }
    }

    Ok(false)
}

async fn fetch_kelas(db: &MySqlPool) -> Vec<KelasRow> {
    sqlx::query_as::<_, KelasRow>(
        r#"
        SELECT CAST(id AS SIGNED) AS id, CAST(nama AS CHAR) AS nama,
               CAST(tingkat AS CHAR) AS tingkat
        FROM kelas
        ORDER BY CAST(tingkat AS UNSIGNED) ASC, nama ASC
        "#,
    )
    .fetch_all(db)
    .await
    .unwrap_or_else(|error| {
        eprintln!("ERROR fetch_kelas: {error:?}");
        Vec::new()
    })
}

async fn find_kelas(db: &MySqlPool, id: i64) -> Result<Option<KelasRow>, sqlx::Error> {
    sqlx::query_as::<_, KelasRow>(
        r#"
        SELECT CAST(id AS SIGNED) AS id, CAST(nama AS CHAR) AS nama,
               CAST(tingkat AS CHAR) AS tingkat
        FROM kelas WHERE id = ? LIMIT 1
        "#,
    )
    .bind(id)
    .fetch_optional(db)
    .await
}

async fn kelas_exists(db: &MySqlPool, id: i64) -> bool {
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM kelas WHERE id = ?")
        .bind(id)
        .fetch_one(db)
        .await
        .unwrap_or(0)
        > 0
}

async fn nama_exists(db: &MySqlPool, nama: &str, except_id: Option<i64>) -> bool {
    let result = match except_id {
        Some(id) => {
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM kelas WHERE LOWER(TRIM(nama)) = LOWER(?) AND id <> ?",
            )
            .bind(nama)
            .bind(id)
            .fetch_one(db)
            .await
        }
        None => {
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM kelas WHERE LOWER(TRIM(nama)) = LOWER(?)",
            )
            .bind(nama)
            .fetch_one(db)
            .await
        }
    };
    result.unwrap_or(0) > 0
}

fn normalize_and_validate(form: KelasForm) -> (String, String, HashMap<String, String>) {
    let nama = form.nama.trim().to_string();
    let tingkat = form.tingkat.trim().to_string();
    let mut errors = HashMap::new();

    if nama.is_empty() {
        errors.insert("nama".into(), "Nama kelas wajib diisi.".into());
    } else if nama.chars().count() > 255 {
        errors.insert("nama".into(), "Nama kelas maksimal 255 karakter.".into());
    }

    match tingkat.parse::<u8>() {
        Ok(value) if (1..=12).contains(&value) => {}
        _ if tingkat.is_empty() => {
            errors.insert("tingkat".into(), "Tingkat wajib diisi.".into());
        }
        _ => {
            errors.insert(
                "tingkat".into(),
                "Tingkat harus berupa angka 1 sampai 12.".into(),
            );
        }
    }

    (nama, tingkat, errors)
}

fn create_form_data(
    nama: String,
    tingkat: String,
    errors: HashMap<String, String>,
) -> KelasFormData {
    KelasFormData {
        nama,
        tingkat,
        errors,
        action: "/data-kelas".into(),
        page_title: "Tambah Data Kelas".into(),
        submit_label: "Simpan".into(),
    }
}

fn edit_form_data(
    id: i64,
    nama: String,
    tingkat: String,
    errors: HashMap<String, String>,
) -> KelasFormData {
    KelasFormData {
        nama,
        tingkat,
        errors,
        action: format!("/data-kelas/{id}"),
        page_title: "Edit Data Kelas".into(),
        submit_label: "Simpan Perubahan".into(),
    }
}

fn render_form_page(ctx: &PageContext, data: KelasFormData) -> Html<String> {
    let title = data.page_title.clone();
    render(ctx, "guru/data_kelas/form.html", &title, data)
}

fn render_form_partial(ctx: &PageContext, data: &KelasFormData) -> String {
    let mut tera_ctx = tera::Context::new();
    tera_ctx.insert("data", data);
    ctx.tera
        .render("guru/data_kelas/_form.html", &tera_ctx)
        .unwrap()
}

fn form_error_response(
    ctx: &PageContext,
    is_htmx: bool,
    data: KelasFormData,
) -> axum::response::Response {
    if is_htmx {
        return (StatusCode::OK, Html(render_form_partial(ctx, &data))).into_response();
    }
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        render_form_page(ctx, data),
    )
        .into_response()
}

fn flash_headers(message: &str, kind: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        "HX-Trigger",
        format!(r#"{{"flash":{{"message":"{message}","type":"{kind}"}}}}"#)
            .parse()
            .unwrap(),
    );
    headers
}

fn success_response(is_htmx: bool, message: &str) -> axum::response::Response {
    if is_htmx {
        let mut headers = flash_headers(message, "success");
        headers.insert("HX-Redirect", "/data-kelas".parse().unwrap());
        return (headers, Html(String::new())).into_response();
    }
    Redirect::to("/data-kelas").into_response()
}

fn error_response(is_htmx: bool, message: &str) -> axum::response::Response {
    if is_htmx {
        return (flash_headers(message, "error"), Html(String::new())).into_response();
    }
    (StatusCode::CONFLICT, message.to_string()).into_response()
}

#[cfg(test)]
mod tests {
    use super::{
        DeleteResult, KelasForm, data_kelas_store, data_kelas_update, delete_kelas_if_unused,
        normalize_and_validate,
    };
    use crate::{
        controllers::guru::absensi_kelas::Htmx,
        models::{auth_user::AuthUser, role::Role},
        utils::page_context::PageContext,
    };
    use axum::{Extension, Form, extract::Path, http::Uri};
    use serial_test::serial;

    fn base_ctx() -> PageContext {
        PageContext {
            user: AuthUser {
                id: 1,
                nis: None,
                roles: vec![Role::Guru],
                name: "Guru".into(),
                foto: None,
            },
            tera: crate::test_support::build_test_tera(),
            uri: Uri::from_static("/data-kelas"),
        }
    }

    #[test]
    fn kelas_form_validates_required_values() {
        assert!(
            !normalize_and_validate(KelasForm {
                nama: String::new(),
                tingkat: "0".into(),
            })
            .2
            .is_empty()
        );
    }

    #[tokio::test]
    #[serial]
    async fn kelas_can_be_created_updated_and_deleted_when_unused() {
        let Some(test_db) = crate::test_support::TestDb::try_new().await else {
            return;
        };

        data_kelas_store(
            base_ctx(),
            Htmx(true),
            Extension(test_db.pool.clone()),
            Form(KelasForm {
                nama: "8 B".into(),
                tingkat: "8".into(),
            }),
        )
        .await;
        let id: i64 = sqlx::query_scalar("SELECT CAST(id AS SIGNED) FROM kelas WHERE nama = '8 B'")
            .fetch_one(&test_db.pool)
            .await
            .unwrap();

        data_kelas_update(
            base_ctx(),
            Htmx(true),
            Path(id),
            Extension(test_db.pool.clone()),
            Form(KelasForm {
                nama: "8 C".into(),
                tingkat: "8".into(),
            }),
        )
        .await;
        let nama: String = sqlx::query_scalar("SELECT nama FROM kelas WHERE id = ?")
            .bind(id)
            .fetch_one(&test_db.pool)
            .await
            .unwrap();
        assert_eq!(nama, "8 C");
        assert!(matches!(
            delete_kelas_if_unused(&test_db.pool, id).await.unwrap(),
            DeleteResult::Deleted
        ));

        test_db.teardown().await;
    }

    #[tokio::test]
    #[serial]
    async fn referenced_kelas_cannot_be_deleted() {
        let Some(test_db) = crate::test_support::TestDb::try_new().await else {
            return;
        };
        let seed = crate::test_support::seed_base_data(&test_db.pool).await;

        assert!(matches!(
            delete_kelas_if_unused(&test_db.pool, seed.kelas_id)
                .await
                .unwrap(),
            DeleteResult::InUse
        ));

        test_db.teardown().await;
    }
}
