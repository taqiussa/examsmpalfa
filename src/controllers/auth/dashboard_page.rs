use axum::response::IntoResponse;
use serde::Serialize;

use crate::utils::{page_context::PageContext, render::render};

#[derive(Serialize)]
struct DashboardData {}

pub async fn dashboard_page(ctx: PageContext) -> impl IntoResponse {
    render(&ctx, "dashboard/index.html", "Dashboard", DashboardData {})
}

#[cfg(test)]
mod tests {
    use super::dashboard_page;
    use crate::{
        models::{auth_user::AuthUser, role::Role},
        utils::page_context::PageContext,
    };
    use axum::http::Uri;
    use axum::response::IntoResponse;

    #[tokio::test]
    async fn dashboard_page_renders() {
        let ctx = PageContext {
            user: AuthUser {
                id: 1,
                roles: vec![Role::Admin],
                name: "Admin".into(),
                foto: None,
            },
            tera: crate::test_support::build_test_tera(),
            uri: Uri::from_static("/dashboard"),
        };

        let response = dashboard_page(ctx).await.into_response();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
    }
}
