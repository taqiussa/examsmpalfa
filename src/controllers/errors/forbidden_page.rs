use axum::response::IntoResponse;
use serde::Serialize;

use crate::utils::{page_context::PageContext, render::render};

#[derive(Serialize)]
struct ForbiddenData {}

pub async fn forbidden_page(ctx: PageContext) -> impl IntoResponse {
    render(&ctx, "errors/403.html", "403 Forbidden", ForbiddenData {})
}

#[cfg(test)]
mod tests {
    use super::forbidden_page;
    use crate::{
        models::{auth_user::AuthUser, role::Role},
        utils::page_context::PageContext,
    };
    use axum::http::Uri;
    use axum::response::IntoResponse;

    #[tokio::test]
    async fn forbidden_page_renders() {
        let ctx = PageContext {
            user: AuthUser {
                id: 1,
                nis: None,
                roles: vec![Role::Admin],
                name: "Admin".into(),
                foto: None,
            },
            tera: crate::test_support::build_test_tera(),
            uri: Uri::from_static("/forbidden"),
        };

        let response = forbidden_page(ctx).await.into_response();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
    }
}
