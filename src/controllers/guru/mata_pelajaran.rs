use axum::{
    Extension, Form,
    extract::Path,
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect},
};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, MySqlPool};
use std::collections::HashMap;

use crate::{
    controllers::guru::absensi_kelas::Htmx,
    utils::{page_context::PageContext, render::render},
};

#[derive(Serialize, FromRow)]
struct MataPelajaranRow {
    id: i64,
    nama: String,
}

#[derive(Serialize)]
struct MataPelajaranListData {
    rows: Vec<MataPelajaranRow>,
}

#[derive(Serialize)]
struct MataPelajaranFormData {
    id: Option<i64>,
    nama: String,
    errors: HashMap<String, String>,
    action: String,
    page_title: String,
    submit_label: String,
}

#[derive(Deserialize)]
pub struct MataPelajaranForm {
    nama: String,
}

pub async fn mata_pelajaran_index(
    ctx: PageContext,
    Extension(db): Extension<MySqlPool>,
) -> Html<String> {
    let data = MataPelajaranListData {
        rows: fetch_mata_pelajaran(&db).await,
    };

    render(
        &ctx,
        "guru/mata_pelajaran/index.html",
        "Mata Pelajaran",
        data,
    )
}

pub async fn mata_pelajaran_create(ctx: PageContext) -> Html<String> {
    render_form_page(&ctx, create_form_data(String::new(), HashMap::new()))
}

pub async fn mata_pelajaran_edit(
    ctx: PageContext,
    Path(id): Path<i64>,
    Extension(db): Extension<MySqlPool>,
) -> axum::response::Response {
    let row = sqlx::query_as::<_, MataPelajaranRow>(
        r#"
        SELECT
            CAST(id AS SIGNED) AS id,
            CAST(nama AS CHAR) AS nama
        FROM mata_pelajarans
        WHERE id = ?
        LIMIT 1
        "#,
    )
    .bind(id)
    .fetch_optional(&db)
    .await;

    match row {
        Ok(Some(row)) => {
            render_form_page(&ctx, edit_form_data(row.id, row.nama, HashMap::new())).into_response()
        }
        Ok(None) => (StatusCode::NOT_FOUND, "Mata pelajaran tidak ditemukan.").into_response(),
        Err(error) => {
            eprintln!("ERROR mata_pelajaran_edit: {error:?}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Gagal mengambil data mata pelajaran.",
            )
                .into_response()
        }
    }
}

pub async fn mata_pelajaran_store(
    ctx: PageContext,
    Htmx(is_htmx): Htmx,
    Extension(db): Extension<MySqlPool>,
    Form(form): Form<MataPelajaranForm>,
) -> axum::response::Response {
    let nama = form.nama.trim().to_string();
    let mut errors = validate_form(&nama);

    if errors.is_empty() && nama_exists(&db, &nama, None).await {
        errors.insert(
            "nama".to_string(),
            "Nama mata pelajaran sudah digunakan.".to_string(),
        );
    }

    if !errors.is_empty() {
        return form_error_response(&ctx, is_htmx, create_form_data(nama, errors));
    }

    match sqlx::query("INSERT INTO mata_pelajarans (nama, kelompok) VALUES (?, 'E')")
        .bind(&nama)
        .execute(&db)
        .await
    {
        Ok(_) => success_response(is_htmx, "Mata pelajaran berhasil ditambahkan."),
        Err(error) => {
            eprintln!("ERROR mata_pelajaran_store: {error:?}");
            let mut errors = HashMap::new();
            errors.insert(
                "nama".to_string(),
                "Mata pelajaran gagal disimpan. Silakan coba lagi.".to_string(),
            );
            form_error_response(&ctx, is_htmx, create_form_data(nama, errors))
        }
    }
}

pub async fn mata_pelajaran_update(
    ctx: PageContext,
    Htmx(is_htmx): Htmx,
    Path(id): Path<i64>,
    Extension(db): Extension<MySqlPool>,
    Form(form): Form<MataPelajaranForm>,
) -> axum::response::Response {
    if !mata_pelajaran_exists(&db, id).await {
        return (StatusCode::NOT_FOUND, "Mata pelajaran tidak ditemukan.").into_response();
    }

    let nama = form.nama.trim().to_string();
    let mut errors = validate_form(&nama);

    if errors.is_empty() && nama_exists(&db, &nama, Some(id)).await {
        errors.insert(
            "nama".to_string(),
            "Nama mata pelajaran sudah digunakan.".to_string(),
        );
    }

    if !errors.is_empty() {
        return form_error_response(&ctx, is_htmx, edit_form_data(id, nama, errors));
    }

    match sqlx::query("UPDATE mata_pelajarans SET nama = ?, kelompok = 'E' WHERE id = ?")
        .bind(&nama)
        .bind(id)
        .execute(&db)
        .await
    {
        Ok(_) => success_response(is_htmx, "Mata pelajaran berhasil diperbarui."),
        Err(error) => {
            eprintln!("ERROR mata_pelajaran_update: {error:?}");
            let mut errors = HashMap::new();
            errors.insert(
                "nama".to_string(),
                "Mata pelajaran gagal diperbarui. Silakan coba lagi.".to_string(),
            );
            form_error_response(&ctx, is_htmx, edit_form_data(id, nama, errors))
        }
    }
}

async fn mata_pelajaran_exists(db: &MySqlPool, id: i64) -> bool {
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM mata_pelajarans WHERE id = ?")
        .bind(id)
        .fetch_one(db)
        .await
        .unwrap_or(0)
        > 0
}

async fn fetch_mata_pelajaran(db: &MySqlPool) -> Vec<MataPelajaranRow> {
    sqlx::query_as::<_, MataPelajaranRow>(
        r#"
        SELECT
            CAST(id AS SIGNED) AS id,
            CAST(nama AS CHAR) AS nama
        FROM mata_pelajarans
        ORDER BY nama ASC
        "#,
    )
    .fetch_all(db)
    .await
    .unwrap_or_else(|error| {
        eprintln!("ERROR fetch_mata_pelajaran: {error:?}");
        Vec::new()
    })
}

async fn nama_exists(db: &MySqlPool, nama: &str, except_id: Option<i64>) -> bool {
    let result = if let Some(id) = except_id {
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM mata_pelajarans WHERE LOWER(TRIM(nama)) = LOWER(?) AND id <> ?",
        )
        .bind(nama)
        .bind(id)
        .fetch_one(db)
        .await
    } else {
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM mata_pelajarans WHERE LOWER(TRIM(nama)) = LOWER(?)",
        )
        .bind(nama)
        .fetch_one(db)
        .await
    };

    result.unwrap_or(0) > 0
}

fn validate_form(nama: &str) -> HashMap<String, String> {
    let mut errors = HashMap::new();
    if nama.trim().is_empty() {
        errors.insert(
            "nama".to_string(),
            "Nama mata pelajaran wajib diisi.".to_string(),
        );
    } else if nama.chars().count() > 255 {
        errors.insert(
            "nama".to_string(),
            "Nama mata pelajaran maksimal 255 karakter.".to_string(),
        );
    }

    errors
}

fn create_form_data(nama: String, errors: HashMap<String, String>) -> MataPelajaranFormData {
    MataPelajaranFormData {
        id: None,
        nama,
        errors,
        action: "/mata-pelajaran".to_string(),
        page_title: "Tambah Mata Pelajaran".to_string(),
        submit_label: "Simpan".to_string(),
    }
}

fn edit_form_data(id: i64, nama: String, errors: HashMap<String, String>) -> MataPelajaranFormData {
    MataPelajaranFormData {
        id: Some(id),
        nama,
        errors,
        action: format!("/mata-pelajaran/{id}"),
        page_title: "Edit Mata Pelajaran".to_string(),
        submit_label: "Simpan Perubahan".to_string(),
    }
}

fn render_form_page(ctx: &PageContext, data: MataPelajaranFormData) -> Html<String> {
    let title = data.page_title.clone();
    render(ctx, "guru/mata_pelajaran/form.html", &title, data)
}

fn render_form_partial(ctx: &PageContext, data: &MataPelajaranFormData) -> String {
    let mut tera_ctx = tera::Context::new();
    tera_ctx.insert("data", data);
    ctx.tera
        .render("guru/mata_pelajaran/_form.html", &tera_ctx)
        .unwrap()
}

fn form_error_response(
    ctx: &PageContext,
    is_htmx: bool,
    data: MataPelajaranFormData,
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

fn success_response(is_htmx: bool, message: &str) -> axum::response::Response {
    if is_htmx {
        let mut headers = HeaderMap::new();
        headers.insert("HX-Redirect", "/mata-pelajaran".parse().unwrap());
        headers.insert(
            "HX-Trigger",
            format!(
                r#"{{"flash":{{"message":"{}","type":"success"}}}}"#,
                message
            )
            .parse()
            .unwrap(),
        );
        return (headers, Html(String::new())).into_response();
    }

    Redirect::to("/mata-pelajaran").into_response()
}

#[cfg(test)]
mod tests {
    use super::{
        MataPelajaranForm, mata_pelajaran_create, mata_pelajaran_index, mata_pelajaran_store,
        mata_pelajaran_update, validate_form,
    };
    use crate::{
        controllers::guru::absensi_kelas::Htmx,
        models::{auth_user::AuthUser, role::Role},
        utils::page_context::PageContext,
    };
    use axum::{
        Extension, Form,
        extract::Path,
        http::{StatusCode, Uri},
        response::IntoResponse,
    };
    use serial_test::serial;

    fn base_ctx() -> PageContext {
        PageContext {
            user: AuthUser {
                id: 1,
                nis: None,
                roles: vec![Role::Guru],
                name: "Guru".to_string(),
                foto: None,
            },
            tera: crate::test_support::build_test_tera(),
            uri: Uri::from_static("/mata-pelajaran/create"),
        }
    }

    #[tokio::test]
    async fn mata_pelajaran_form_validates_required_values_and_defaults_to_e() {
        assert!(validate_form("").contains_key("nama"));
        assert!(validate_form("Matematika").is_empty());

        let page = mata_pelajaran_create(base_ctx()).await;
        assert!(page.0.contains("name=\"kelompok\" value=\"E\""));
    }

    #[tokio::test]
    #[serial]
    async fn mata_pelajaran_can_be_created_and_updated() {
        let Some(test_db) = crate::test_support::TestDb::try_new().await else {
            return;
        };

        let create_response = mata_pelajaran_store(
            base_ctx(),
            Htmx(true),
            Extension(test_db.pool.clone()),
            Form(MataPelajaranForm {
                nama: "Matematika".to_string(),
            }),
        )
        .await
        .into_response();

        assert_eq!(create_response.status(), StatusCode::OK);
        assert_eq!(
            create_response
                .headers()
                .get("HX-Redirect")
                .and_then(|value| value.to_str().ok()),
            Some("/mata-pelajaran")
        );

        let id: i64 = sqlx::query_scalar(
            "SELECT CAST(id AS SIGNED) FROM mata_pelajarans WHERE nama = ? LIMIT 1",
        )
        .bind("Matematika")
        .fetch_one(&test_db.pool)
        .await
        .expect("created subject should exist");

        let update_response = mata_pelajaran_update(
            base_ctx(),
            Htmx(true),
            Path(id),
            Extension(test_db.pool.clone()),
            Form(MataPelajaranForm {
                nama: "Matematika Lanjutan".to_string(),
            }),
        )
        .await
        .into_response();

        assert_eq!(update_response.status(), StatusCode::OK);
        let (nama, kelompok): (String, String) =
            sqlx::query_as("SELECT nama, kelompok FROM mata_pelajarans WHERE id = ? LIMIT 1")
                .bind(id)
                .fetch_one(&test_db.pool)
                .await
                .expect("updated subject should exist");
        assert_eq!(nama, "Matematika Lanjutan");
        assert_eq!(kelompok, "E");

        let list = mata_pelajaran_index(base_ctx(), Extension(test_db.pool.clone())).await;
        assert!(list.0.contains("Matematika Lanjutan"));
        assert!(!list.0.contains("Kelompok</th>"));

        test_db.teardown().await;
    }
}
