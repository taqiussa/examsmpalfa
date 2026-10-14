use axum::{
    body::Body,
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};

use crate::models::auth_user::{AccountType, AuthUser};

#[derive(Clone)]
pub struct AllowedAccountTypes(pub Vec<AccountType>);

pub async fn account_middleware(req: Request<Body>, next: Next) -> Response {
    let Some(user) = req.extensions().get::<AuthUser>() else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Some(allowed) = req.extensions().get::<AllowedAccountTypes>() else {
        return next.run(req).await;
    };

    if !allowed.0.contains(&user.account_type) {
        return StatusCode::FORBIDDEN.into_response();
    }

    next.run(req).await
}

#[cfg(test)]
mod tests {
    use super::{AllowedAccountTypes, account_middleware};
    use crate::models::auth_user::{AccountType, AuthUser};
    use axum::{
        Extension, Router,
        body::Body,
        http::{Request, StatusCode},
        middleware,
        routing::get,
    };
    use tower::ServiceExt;

    fn user(account_type: AccountType) -> AuthUser {
        AuthUser {
            id: 1,
            account_type,
            nis: (account_type == AccountType::Siswa).then(|| "12345".into()),
            roles: Vec::new(),
            name: "User".into(),
            foto: None,
        }
    }

    #[tokio::test]
    async fn only_allows_the_configured_account_type() {
        let app = Router::new()
            .route("/siswa", get(|| async { "ok" }))
            .route_layer(middleware::from_fn(account_middleware))
            .layer(Extension(AllowedAccountTypes(vec![AccountType::Siswa])))
            .layer(Extension(user(AccountType::Web)));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/siswa")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
}
