use axum::{
    Extension, Form,
    extract::FromRequestParts,
    http::{HeaderMap, StatusCode, request::Parts},
    response::{Html, IntoResponse},
};
use bcrypt::{DEFAULT_COST, hash};
use serde::{Deserialize, Serialize};
use sqlx::MySqlPool;
use std::collections::HashMap;

use crate::utils::{page_context::PageContext, render::render};
use tera::Context as TeraContext;

#[derive(Serialize)]
struct TambahPenggunaData {
    name: String,
    username: String,
    errors: HashMap<String, String>,
}

#[derive(Serialize)]
struct UserRow {
    id: i64,
    name: String,
    username: String,
}

#[derive(Deserialize)]
pub struct TambahPenggunaForm {
    name: String,
    username: String,
    password: String,
    password_confirmation: String,
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

pub async fn tambah_pengguna_page(ctx: PageContext) -> Html<String> {
    let data = TambahPenggunaData {
        name: "".to_string(),
        username: "".to_string(),
        errors: HashMap::new(),
    };

    render(&ctx, "admin/tambah_pengguna.html", "Tambah Pengguna", data)
}

pub async fn tambah_pengguna_action(
    ctx: PageContext,
    Htmx(is_htmx): Htmx,
    Extension(db): Extension<MySqlPool>,
    Form(payload): Form<TambahPenggunaForm>,
) -> impl IntoResponse {
    let mut errors = HashMap::new();

    if payload.name.trim().is_empty() {
        errors.insert("name".to_string(), "Nama tidak boleh kosong".to_string());
    }
    if payload.username.trim().is_empty() {
        errors.insert(
            "username".to_string(),
            "Username tidak boleh kosong".to_string(),
        );
    }
    if payload.password.trim().is_empty() {
        errors.insert(
            "password".to_string(),
            "Password tidak boleh kosong".to_string(),
        );
    }
    if payload.password_confirmation.trim().is_empty() {
        errors.insert(
            "password_confirmation".to_string(),
            "Konfirmasi password tidak boleh kosong".to_string(),
        );
    }
    if !payload.password.is_empty() && payload.password != payload.password_confirmation {
        errors.insert(
            "password_confirmation".to_string(),
            "Konfirmasi password tidak sama".to_string(),
        );
    }

    if errors.is_empty() {
        let exists = sqlx::query_scalar!(
            r#"SELECT COUNT(*) as "count!" FROM users WHERE username = ?"#,
            payload.username
        )
        .fetch_one(&db)
        .await
        .unwrap_or(0);

        if exists > 0 {
            errors.insert(
                "username".to_string(),
                "Username sudah digunakan".to_string(),
            );
        }
    }

    if !errors.is_empty() {
        let data = TambahPenggunaData {
            name: payload.name,
            username: payload.username,
            errors,
        };
        if is_htmx {
            return (StatusCode::OK, Html(render_form_partial(&ctx, &data))).into_response();
        }
        return (
            StatusCode::BAD_REQUEST,
            render(&ctx, "admin/tambah_pengguna.html", "Tambah Pengguna", data),
        )
            .into_response();
    }

    let hashed = hash(payload.password, DEFAULT_COST).unwrap();

    let result = sqlx::query!(
        r#"
        INSERT INTO users (name, username, password, created_at, updated_at)
        VALUES (?, ?, ?, NOW(), NOW())
        "#,
        payload.name,
        payload.username,
        hashed
    )
    .execute(&db)
    .await;

    let _ = result;

    let data = TambahPenggunaData {
        name: "".to_string(),
        username: "".to_string(),
        errors: HashMap::new(),
    };

    if is_htmx {
        let mut headers = HeaderMap::new();
        headers.insert(
            "HX-Trigger",
            r#"{"flash":{"message":"Berhasil menambahkan pengguna","type":"success"},"pengguna:changed":true}"#
                .parse()
                .unwrap(),
        );
        let html = render_form_partial(&ctx, &data);
        return (headers, Html(html)).into_response();
    }

    render(&ctx, "admin/tambah_pengguna.html", "Tambah Pengguna", data).into_response()
}

#[derive(Deserialize)]
pub struct HapusPenggunaForm {
    id: i64,
}

pub async fn pengguna_table(ctx: PageContext, Extension(db): Extension<MySqlPool>) -> Html<String> {
    let users = fetch_users(&db).await;
    Html(render_table_partial(&ctx, &users))
}

pub async fn hapus_pengguna_action(
    ctx: PageContext,
    Htmx(is_htmx): Htmx,
    Extension(db): Extension<MySqlPool>,
    Form(payload): Form<HapusPenggunaForm>,
) -> impl IntoResponse {
    if !is_htmx {
        return StatusCode::BAD_REQUEST.into_response();
    }

    let _ = sqlx::query!("DELETE FROM users WHERE id = ?", payload.id)
        .execute(&db)
        .await;

    let users = fetch_users(&db).await;
    let mut headers = HeaderMap::new();
    headers.insert(
        "HX-Trigger",
        r#"{"flash":{"message":"Pengguna berhasil dihapus","type":"success"}}"#
            .parse()
            .unwrap(),
    );
    (headers, Html(render_table_partial(&ctx, &users))).into_response()
}

fn render_form_partial(ctx: &PageContext, data: &TambahPenggunaData) -> String {
    let mut tera_ctx = TeraContext::new();
    tera_ctx.insert("form_name", &data.name);
    tera_ctx.insert("form_username", &data.username);
    tera_ctx.insert("form_errors", &data.errors);
    ctx.tera
        .render("admin/_tambah_pengguna_form.html", &tera_ctx)
        .unwrap()
}

fn render_table_partial(ctx: &PageContext, users: &[UserRow]) -> String {
    let mut tera_ctx = TeraContext::new();
    tera_ctx.insert("users", users);
    ctx.tera
        .render("admin/_pengguna_table.html", &tera_ctx)
        .unwrap()
}

async fn fetch_users(db: &MySqlPool) -> Vec<UserRow> {
    sqlx::query_as!(
        UserRow,
        r#"
        SELECT 
            CAST(id AS SIGNED) AS id,
            name,
            username as "username!"
        FROM users
        WHERE username IS NOT NULL
        ORDER BY name ASC
        "#
    )
    .fetch_all(db)
    .await
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{
        hapus_pengguna_action, tambah_pengguna_action, HapusPenggunaForm, Htmx,
        TambahPenggunaForm,
    };
    use crate::{
        models::{auth_user::AuthUser, role::Role},
        utils::page_context::PageContext,
    };
    use axum::{Extension, Form};
    use axum::http::Uri;
    use axum::response::IntoResponse;
    use serial_test::serial;

    fn base_ctx() -> PageContext {
        PageContext {
            user: AuthUser {
                id: 1,
                nis: None,
                roles: vec![Role::Admin],
                name: "Admin".into(),
                foto: None,
            },
            tera: crate::test_support::build_test_tera(),
            uri: Uri::from_static("/tambah-pengguna"),
        }
    }

    #[tokio::test]
    #[serial]
    async fn tambah_pengguna_validates_fields() {
        let Some(test_db) = crate::test_support::TestDb::try_new().await else {
            return;
        };
        crate::test_support::seed_base_data(&test_db.pool).await;

        let form = TambahPenggunaForm {
            name: "".into(),
            username: "".into(),
            password: "".into(),
            password_confirmation: "".into(),
        };

        let response = tambah_pengguna_action(
            base_ctx(),
            Htmx(false),
            Extension(test_db.pool.clone()),
            Form(form),
        )
        .await
        .into_response();

        assert_eq!(response.status(), axum::http::StatusCode::BAD_REQUEST);
        test_db.teardown().await;
    }

    #[tokio::test]
    #[serial]
    async fn tambah_pengguna_inserts_user_when_valid_htmx() {
        let Some(test_db) = crate::test_support::TestDb::try_new().await else {
            return;
        };
        crate::test_support::seed_base_data(&test_db.pool).await;

        let form = TambahPenggunaForm {
            name: "User Baru".into(),
            username: "baru".into(),
            password: "secret123".into(),
            password_confirmation: "secret123".into(),
        };

        let response = tambah_pengguna_action(
            base_ctx(),
            Htmx(true),
            Extension(test_db.pool.clone()),
            Form(form),
        )
        .await
        .into_response();

        assert_eq!(response.status(), axum::http::StatusCode::OK);
        assert!(response.headers().get("HX-Trigger").is_some());

        let count: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM users WHERE username = 'baru'")
            .fetch_one(&test_db.pool)
            .await
            .unwrap_or(0);
        assert_eq!(count, 1);

        test_db.teardown().await;
    }

    #[tokio::test]
    #[serial]
    async fn hapus_pengguna_requires_htmx_and_deletes() {
        let Some(test_db) = crate::test_support::TestDb::try_new().await else {
            return;
        };
        let seed = crate::test_support::seed_base_data(&test_db.pool).await;

        let form = HapusPenggunaForm {
            id: seed.admin_id as i64,
        };

        let response = hapus_pengguna_action(
            base_ctx(),
            Htmx(true),
            Extension(test_db.pool.clone()),
            Form(form),
        )
        .await
        .into_response();

        assert_eq!(response.status(), axum::http::StatusCode::OK);

        let count: i64 = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM users WHERE id = ?",
            seed.admin_id as i64
        )
        .fetch_one(&test_db.pool)
        .await
        .unwrap_or(0);
        assert_eq!(count, 0);

        test_db.teardown().await;
    }
}
