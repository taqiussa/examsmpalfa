use axum::{
    Extension, Form,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use bcrypt::verify;
use serde::Deserialize;
use sqlx::MySqlPool;

use crate::models::auth_user::AccountType;

const AUTH_COOKIE: &str = "auth_identity";

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

struct LoginIdentity {
    user: User,
    account_type: AccountType,
}

pub async fn login_action(
    jar: CookieJar,
    Extension(db): Extension<MySqlPool>,
    Form(payload): Form<LoginForm>,
) -> impl IntoResponse {
    let username = payload.username.trim();
    let identity = if username.chars().all(|c| c.is_ascii_digit()) {
        let Ok(nis) = username.parse::<u32>() else {
            return invalid_credentials();
        };
        match sqlx::query_as::<_, User>("SELECT id, password FROM user_siswa WHERE nis = ? LIMIT 1")
            .bind(nis)
            .fetch_optional(&db)
            .await
        {
            Ok(user) => user.map(|user| LoginIdentity {
                user,
                account_type: AccountType::Siswa,
            }),
            Err(error) => {
                eprintln!("ERROR login siswa query: {error:?}");
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        }
    } else {
        match sqlx::query_as::<_, User>("SELECT id, password FROM users WHERE username = ? LIMIT 1")
            .bind(username)
            .fetch_optional(&db)
            .await
        {
            Ok(user) => user.map(|user| LoginIdentity {
                user,
                account_type: AccountType::Web,
            }),
            Err(error) => {
                eprintln!("ERROR login web query: {error:?}");
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        }
    };

    let Some(identity) = identity else {
        return invalid_credentials();
    };

    let valid = verify(&payload.password, &identity.user.password).unwrap_or(false);

    if !valid {
        return invalid_credentials();
    }

    let cookie = Cookie::build((
        AUTH_COOKIE,
        format!(
            "{}.{}",
            identity.account_type.cookie_value(),
            identity.user.id
        ),
    ))
    .path("/")
    .http_only(true)
    .same_site(SameSite::Lax)
    .build();

    let jar = jar.add(cookie);

    let mut headers = HeaderMap::new();
    let redirect = match identity.account_type {
        AccountType::Web => "/dashboard",
        AccountType::Siswa => "/siswa/ujian",
    };
    headers.insert("HX-Redirect", redirect.parse().unwrap());

    (jar, headers, StatusCode::OK).into_response()
}

fn invalid_credentials() -> axum::response::Response {
    (
        StatusCode::OK,
        "<div class='text-red-600'>Username atau password salah</div>",
    )
        .into_response()
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
        let cookie = response
            .headers()
            .get("set-cookie")
            .unwrap()
            .to_str()
            .unwrap();
        assert!(cookie.contains("auth_identity=web.1"));

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
        let cookie = response
            .headers()
            .get("set-cookie")
            .unwrap()
            .to_str()
            .unwrap();
        assert!(cookie.contains("auth_identity=siswa.1"));

        test_db.teardown().await;
    }
}
