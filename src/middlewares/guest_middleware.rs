use axum::{
    body::Body,
    http::Request,
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
};
use axum_extra::extract::cookie::CookieJar;

pub async fn guest_middleware(jar: CookieJar, req: Request<Body>, next: Next) -> Response {
    if jar.get("user_id").is_some() {
        // sudah login → jangan ke login page
        return Redirect::to("/dashboard").into_response();
    }

    next.run(req).await
}

#[cfg(test)]
mod tests {
    use super::guest_middleware;
    use axum::{
        Router,
        body::Body,
        http::{Request, StatusCode},
        middleware,
        response::IntoResponse,
        routing::get,
    };
    use tower::ServiceExt;

    async fn ok_handler() -> impl IntoResponse {
        "ok"
    }

    #[tokio::test]
    async fn guest_middleware_allows_guest() {
        let app = Router::new()
            .route("/login", get(ok_handler))
            .layer(middleware::from_fn(guest_middleware));

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
    }

    #[tokio::test]
    async fn guest_middleware_redirects_logged_in() {
        let app = Router::new()
            .route("/login", get(ok_handler))
            .layer(middleware::from_fn(guest_middleware));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/login")
                    .header("cookie", "user_id=1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::SEE_OTHER);
    }
}
