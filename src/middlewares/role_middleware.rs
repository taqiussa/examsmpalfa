use crate::{
    layouts::sidebar::LayoutContext,
    models::{auth_user::AuthUser, role::Role},
    utils::vite,
};
use axum::{
    body::Body,
    http::{Request, StatusCode},
    middleware::Next,
    response::{Html, IntoResponse, Response},
};
use tera::{Context, Tera};

#[derive(Clone)]
pub struct AllowedRoles(pub Vec<Role>);

pub async fn role_middleware(req: Request<Body>, next: Next) -> Response {
    let user = match req.extensions().get::<AuthUser>() {
        Some(u) => u.clone(),
        None => return StatusCode::UNAUTHORIZED.into_response(),
    };

    let allowed = match req.extensions().get::<AllowedRoles>() {
        Some(a) => a,
        None => return next.run(req).await,
    };

    let is_allowed = user.roles.iter().any(|r| allowed.0.contains(r));

    if !is_allowed {
        let tera = req.extensions().get::<Tera>().unwrap().clone();

        let mut ctx = Context::new();
        ctx.insert("title", "403 Forbidden");
        ctx.insert("layout", &LayoutContext::with_user(&user, req.uri().path()));
        ctx.insert("vite_css", &vite::vite_css());
        ctx.insert("vite_js", &vite::vite_js());

        let rendered = tera.render("errors/403.html", &ctx).unwrap();

        return (StatusCode::FORBIDDEN, Html(rendered)).into_response();
    }

    next.run(req).await
}

#[cfg(test)]
mod tests {
    use super::{role_middleware, AllowedRoles};
    use crate::models::{auth_user::AuthUser, role::Role};
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
        middleware,
        response::IntoResponse,
        routing::get,
        Extension, Router,
    };
    use tower::ServiceExt;

    async fn ok_handler() -> impl IntoResponse {
        "ok"
    }

    #[tokio::test]
    async fn role_middleware_allows_matching_role() {
        let app = Router::new()
            .route("/admin", get(ok_handler))
            .layer(middleware::from_fn(role_middleware))
            .layer(Extension(crate::test_support::build_test_tera()))
            .layer(Extension(AuthUser {
                id: 1,
                roles: vec![Role::Admin],
                name: "Admin".into(),
                foto: None,
            }))
            .layer(Extension(AllowedRoles(vec![Role::Admin])));

        let response = app
            .oneshot(Request::builder().uri("/admin").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn role_middleware_blocks_non_matching_role() {
        let app = Router::new()
            .route("/admin", get(ok_handler))
            .layer(middleware::from_fn(role_middleware))
            .layer(Extension(crate::test_support::build_test_tera()))
            .layer(Extension(AuthUser {
                id: 2,
                roles: vec![Role::Guru],
                name: "Guru".into(),
                foto: None,
            }))
            .layer(Extension(AllowedRoles(vec![Role::Admin])));

        let response = app
            .oneshot(Request::builder().uri("/admin").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body_str = String::from_utf8_lossy(&body);
        assert!(body_str.contains("403"));
    }
}
