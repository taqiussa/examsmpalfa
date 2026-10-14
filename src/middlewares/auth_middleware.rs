use axum::{
    body::Body,
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
};
use axum_extra::extract::cookie::CookieJar;
use sqlx::{FromRow, MySqlPool};

use crate::models::auth_user::{AccountType, AuthUser};
use crate::models::role::Role;

#[derive(FromRow)]
struct RoleRow {
    name: String,
}

#[derive(FromRow)]
struct AuthMiddlewareUserRow {
    name: String,
    foto: Option<String>,
    nis: Option<String>,
}

pub async fn auth_middleware(jar: CookieJar, mut req: Request<Body>, next: Next) -> Response {
    let (account_type, user_id) = match jar.get("auth_identity") {
        Some(cookie) => match parse_identity(cookie.value()) {
            Some(identity) => identity,
            None => return redirect_login(&req),
        },
        None => return redirect_login(&req),
    };

    // 2️⃣ ambil DB pool
    let pool = req
        .extensions()
        .get::<MySqlPool>()
        .expect("MySqlPool missing");

    let (user, roles) = match account_type {
        AccountType::Web => {
            let user = match sqlx::query_as::<_, AuthMiddlewareUserRow>(
                "SELECT name, foto, NULL AS nis FROM users WHERE id = ?",
            )
            .bind(user_id)
            .fetch_optional(pool)
            .await
            {
                Ok(Some(user)) => user,
                Ok(None) => return redirect_login(&req),
                Err(error) => {
                    eprintln!("ERROR auth web user query: {error:?}");
                    return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                }
            };
            let roles = match sqlx::query_as::<_, RoleRow>(
                r#"
                SELECT r.name
                FROM roles r
                JOIN model_has_roles mhr ON mhr.role_id = r.id
                WHERE mhr.model_id = ?
                  AND mhr.model_type = 'App\\Models\\User'
                  AND r.guard_name = 'web'
                "#,
            )
            .bind(user_id)
            .fetch_all(pool)
            .await
            {
                Ok(rows) => rows.into_iter().map(|row| Role::from(&row.name)).collect(),
                Err(error) => {
                    eprintln!("ERROR auth web roles query: {error:?}");
                    return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                }
            };
            (user, roles)
        }
        AccountType::Siswa => {
            let user = match sqlx::query_as::<_, AuthMiddlewareUserRow>(
                "SELECT name, NULL AS foto, CAST(nis AS CHAR) AS nis FROM user_siswa WHERE id = ?",
            )
            .bind(user_id)
            .fetch_optional(pool)
            .await
            {
                Ok(Some(user)) => user,
                Ok(None) => return redirect_login(&req),
                Err(error) => {
                    eprintln!("ERROR auth siswa query: {error:?}");
                    return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                }
            };
            (user, Vec::new())
        }
    };

    req.extensions_mut().insert(AuthUser {
        id: user_id,
        account_type,
        nis: user.nis,
        roles,
        name: user.name,
        foto: user.foto,
    });

    next.run(req).await
}

fn parse_identity(value: &str) -> Option<(AccountType, u64)> {
    let (kind, id) = value.split_once('.')?;
    Some((AccountType::from_cookie(kind)?, id.parse().ok()?))
}

fn redirect_login(req: &Request<Body>) -> Response {
    if req.headers().contains_key("hx-request") {
        (StatusCode::UNAUTHORIZED, [("HX-Redirect", "/login")]).into_response()
    } else {
        Redirect::to("/login").into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::auth_middleware;
    use axum::{
        Extension, Router,
        body::Body,
        http::{Request, StatusCode},
        middleware,
        response::IntoResponse,
        routing::get,
    };
    use serial_test::serial;
    use tower::ServiceExt;

    async fn protected_handler(
        axum::Extension(user): Extension<crate::models::auth_user::AuthUser>,
    ) -> impl IntoResponse {
        format!("hello {}", user.name)
    }

    #[tokio::test]
    #[serial]
    async fn auth_middleware_redirects_without_cookie() {
        let Some(test_db) = crate::test_support::TestDb::try_new().await else {
            return;
        };
        let app = Router::new()
            .route("/protected", get(protected_handler))
            .layer(middleware::from_fn(auth_middleware))
            .layer(Extension(test_db.pool.clone()));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/protected")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        test_db.teardown().await;
    }

    #[tokio::test]
    #[serial]
    async fn auth_middleware_injects_user() {
        let Some(test_db) = crate::test_support::TestDb::try_new().await else {
            return;
        };
        let seed = crate::test_support::seed_base_data(&test_db.pool).await;

        let app = Router::new()
            .route("/protected", get(protected_handler))
            .layer(middleware::from_fn(auth_middleware))
            .layer(Extension(test_db.pool.clone()));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/protected")
                    .header("cookie", format!("auth_identity=web.{}", seed.admin_id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        test_db.teardown().await;
    }

    #[tokio::test]
    #[serial]
    async fn auth_middleware_distinguishes_colliding_ids() {
        let Some(test_db) = crate::test_support::TestDb::try_new().await else {
            return;
        };
        let seed = crate::test_support::seed_base_data(&test_db.pool).await;
        assert_eq!(seed.admin_id, seed.siswa_id);

        let app = Router::new()
            .route("/protected", get(protected_handler))
            .layer(middleware::from_fn(auth_middleware))
            .layer(Extension(test_db.pool.clone()));

        let web = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/protected")
                    .header("cookie", format!("auth_identity=web.{}", seed.admin_id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let siswa = app
            .oneshot(
                Request::builder()
                    .uri("/protected")
                    .header("cookie", format!("auth_identity=siswa.{}", seed.siswa_id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(web.status(), StatusCode::OK);
        assert_eq!(siswa.status(), StatusCode::OK);
        test_db.teardown().await;
    }
}
