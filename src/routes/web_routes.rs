use axum::{Router, middleware};

use crate::{
    controllers::errors::{forbidden_page::forbidden_page, not_found_page::not_found_page},
    middlewares::auth_middleware::auth_middleware,
    routes::{
        admin_routes::admin_routes,
        auth_routes::auth_routes,
        guru_routes::{guru_only_routes, guru_routes},
        public_routes::public_routes,
        siswa_routes::siswa_routes,
    },
};

pub fn web_routes() -> Router {
    // 🌐 PUBLIC (guest)
    let public = public_routes();

    // 🔐 AUTHENTICATED (common)
    let authenticated = Router::new()
        // common authenticated routes
        .merge(auth_routes())
        // ❌ forbidden page (authenticated, NO role check)
        .route("/forbidden", axum::routing::get(forbidden_page))
        // role-based areas
        .merge(admin_routes())
        .merge(guru_only_routes())
        .merge(guru_routes())
        .merge(siswa_routes())
        .fallback(axum::routing::get(not_found_page))
        .layer(middleware::from_fn(auth_middleware));

    Router::new().merge(public).merge(authenticated)
}

#[cfg(test)]
mod tests {
    use super::web_routes;
    use axum::{
        Extension, Router,
        body::Body,
        http::{Request, StatusCode},
    };
    use serial_test::serial;
    use tower::ServiceExt;

    fn build_app(pool: sqlx::MySqlPool) -> Router {
        Router::new()
            .merge(web_routes())
            .layer(Extension(pool))
            .layer(Extension(crate::test_support::build_test_tera()))
    }

    #[tokio::test]
    #[serial]
    async fn public_login_route_is_accessible() {
        let Some(test_db) = crate::test_support::TestDb::try_new().await else {
            return;
        };
        let app = build_app(test_db.pool.clone());

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/login")
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
    async fn dashboard_requires_auth() {
        let Some(test_db) = crate::test_support::TestDb::try_new().await else {
            return;
        };
        let app = build_app(test_db.pool.clone());

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/dashboard")
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
    async fn role_routes_enforce_forbidden() {
        let Some(test_db) = crate::test_support::TestDb::try_new().await else {
            return;
        };
        let seed = crate::test_support::seed_base_data(&test_db.pool).await;
        let app = build_app(test_db.pool.clone());

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/tambah-pengguna")
                    .header("cookie", format!("user_id={}", seed.guru_id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);

        test_db.teardown().await;
    }

    #[tokio::test]
    #[serial]
    async fn upload_peserta_is_available_to_guru_and_forbidden_to_admin() {
        let Some(test_db) = crate::test_support::TestDb::try_new().await else {
            return;
        };
        let seed = crate::test_support::seed_base_data(&test_db.pool).await;
        let app = build_app(test_db.pool.clone());

        let guru_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/upload-peserta")
                    .header("cookie", format!("user_id={}", seed.guru_id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(guru_response.status(), StatusCode::OK);

        let admin_response = app
            .oneshot(
                Request::builder()
                    .uri("/upload-peserta")
                    .header("cookie", format!("user_id={}", seed.admin_id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(admin_response.status(), StatusCode::FORBIDDEN);

        test_db.teardown().await;
    }
}
