use axum::{
    Extension, Form,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use axum_extra::extract::cookie::{Cookie, CookieJar};
use bcrypt::verify;
use serde::Deserialize;
use sqlx::MySqlPool;

#[derive(Deserialize)]
pub struct LoginForm {
    pub username: String,
    pub password: String,
}

#[derive(sqlx::FromRow)]
struct User {
    id: u64,
    password: String,
}

pub async fn login_action(
    jar: CookieJar,
    Extension(db): Extension<MySqlPool>,
    Form(payload): Form<LoginForm>,
) -> impl IntoResponse {
    // 1. Cari user berdasarkan input: jika angka -> pakai nis, jika string -> pakai username
    let is_numeric = payload.username.chars().all(|c| c.is_ascii_digit());
    let user = if is_numeric {
        sqlx::query_as::<_, User>("SELECT id, password FROM users WHERE nis = ? LIMIT 1")
            .bind(&payload.username)
            .fetch_optional(&db)
            .await
            .unwrap()
    } else {
        sqlx::query_as::<_, User>("SELECT id, password FROM users WHERE username = ? LIMIT 1")
            .bind(&payload.username)
            .fetch_optional(&db)
            .await
            .unwrap()
    };

    // 2. Jika user tidak ada → error GENERIK (AMAN)
    let user = match user {
        Some(u) => u,
        None => {
            return (
                StatusCode::OK,
                "<div class='text-red-600'>Username atau password salah</div>",
            )
                .into_response();
        }
    };

    // 3. Verifikasi password (bcrypt)
    let valid = verify(&payload.password, &user.password).unwrap_or(false);

    if !valid {
        return (
            StatusCode::OK,
            "<div class='text-red-600'>Username atau password salah</div>",
        )
            .into_response();
    }

    // 4. Login sukses → simpan session di cookie
    let mut cookie = Cookie::new("user_id", user.id.to_string());
    cookie.set_path("/");
    cookie.set_http_only(true);

    let jar = jar.add(cookie);

    // 5. HTMX redirect ke dashboard
    let mut headers = HeaderMap::new();
    headers.insert("HX-Redirect", "/dashboard".parse().unwrap());

    (jar, headers, StatusCode::OK).into_response()
}

#[cfg(test)]
mod tests {
    use super::{LoginForm, login_action};
    use axum::response::IntoResponse;
    use axum::{Extension, Form};
    use axum_extra::extract::cookie::CookieJar;
    use serial_test::serial;

    #[tokio::test]
    #[serial]
    async fn login_action_accepts_username_and_sets_cookie() {
        let Some(test_db) = crate::test_support::TestDb::try_new().await else {
            return;
        };
        crate::test_support::seed_base_data(&test_db.pool).await;

        let form = LoginForm {
            username: "admin".into(),
            password: "secret123".into(),
        };
        let response = login_action(
            CookieJar::new(),
            Extension(test_db.pool.clone()),
            Form(form),
        )
        .await;

        let response = response.into_response();
        assert!(response.headers().get("set-cookie").is_some());

        test_db.teardown().await;
    }

    #[tokio::test]
    #[serial]
    async fn login_action_accepts_nis() {
        let Some(test_db) = crate::test_support::TestDb::try_new().await else {
            return;
        };
        let seed = crate::test_support::seed_base_data(&test_db.pool).await;

        let form = LoginForm {
            username: seed.nis,
            password: "secret123".into(),
        };
        let response = login_action(
            CookieJar::new(),
            Extension(test_db.pool.clone()),
            Form(form),
        )
        .await;

        let response = response.into_response();
        assert!(response.headers().get("set-cookie").is_some());

        test_db.teardown().await;
    }
}
