use axum::{
    body::Body,
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
};
use axum_extra::extract::cookie::CookieJar;
use sqlx::MySqlPool;

use crate::models::auth_user::AuthUser;
use crate::models::role::Role;

pub async fn auth_middleware(jar: CookieJar, mut req: Request<Body>, next: Next) -> Response {
    // 1️⃣ cek cookie login
    let user_id: u64 = match jar.get("user_id") {
        Some(c) => match c.value().parse() {
            Ok(id) => id,
            Err(_) => return redirect_login(&req),
        },
        None => return redirect_login(&req),
    };

    // 2️⃣ ambil DB pool
    let pool = req
        .extensions()
        .get::<MySqlPool>()
        .expect("MySqlPool missing");

    // 3️⃣ ambil semua role dari Spatie tables
    let roles: Vec<Role> = match sqlx::query!(
        r#"
        SELECT r.name
        FROM roles r
        JOIN model_has_roles mhr ON mhr.role_id = r.id
        WHERE mhr.model_id = ?
        "#,
        user_id
    )
    .fetch_all(pool)
    .await
    {
        Ok(rows) => rows.into_iter().map(|r| Role::from(&r.name)).collect(),
        Err(_) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to fetch roles").into_response();
        }
    };

    // 4️⃣ ambil data user
    let user = match sqlx::query!("SELECT name, foto, nis FROM users WHERE id = ?", user_id)
        .fetch_one(pool)
        .await
    {
        Ok(u) => u,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "User not found").into_response(),
    };

    // 5️⃣ inject AuthUser dengan multi-role
    req.extensions_mut().insert(AuthUser {
        id: user_id,
        nis: user.nis,
        roles, // ⬅️ pakai Vec<Role>
        name: user.name,
        foto: user.foto,
    });

    // 6️⃣ lanjut
    next.run(req).await
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
        body::Body,
        http::{Request, StatusCode},
        middleware,
        response::IntoResponse,
        routing::get,
        Extension, Router,
    };
    use serial_test::serial;
    use tower::ServiceExt;

    async fn protected_handler(axum::Extension(user): Extension<crate::models::auth_user::AuthUser>) -> impl IntoResponse {
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
            .layer(Extension(test_db.pool.clone()))
            .layer(middleware::from_fn(auth_middleware));

        let response = app
            .oneshot(Request::builder().uri("/protected").body(Body::empty()).unwrap())
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
            .layer(Extension(test_db.pool.clone()))
            .layer(middleware::from_fn(auth_middleware));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/protected")
                    .header("cookie", format!("user_id={}", seed.admin_id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        test_db.teardown().await;
    }
}
