use axum::{
    extract::FromRequestParts,
    http::{StatusCode, Uri, request::Parts},
};
use tera::Tera;

use crate::models::auth_user::AuthUser;

#[derive(Clone)]
pub struct PageContext {
    pub user: AuthUser,
    pub tera: Tera,
    pub uri: Uri,
}

impl<S> FromRequestParts<S> for PageContext
where
    S: Send + Sync,
{
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let user = parts
            .extensions
            .get::<AuthUser>()
            .ok_or(StatusCode::UNAUTHORIZED)?
            .clone();

        let tera = parts
            .extensions
            .get::<Tera>()
            .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?
            .clone();

        let uri = parts.uri.clone();

        Ok(Self { user, tera, uri })
    }
}

#[cfg(test)]
mod tests {
    use super::PageContext;
    use crate::models::{auth_user::AuthUser, role::Role};
    use axum::{
        body::Body,
        extract::FromRequestParts,
        http::{Request, StatusCode},
    };

    #[tokio::test]
    async fn from_request_parts_requires_user_and_tera() {
        let req = Request::new(Body::empty());
        let (mut parts, _body) = req.into_parts();
        let state = ();

        let result = PageContext::from_request_parts(&mut parts, &state).await;
        assert!(matches!(result, Err(StatusCode::UNAUTHORIZED)));

        parts.extensions.insert(AuthUser {
            id: 1,
            nis: None,
            roles: vec![Role::Admin],
            name: "Admin".into(),
            foto: None,
        });

        let result = PageContext::from_request_parts(&mut parts, &state).await;
        assert!(matches!(result, Err(StatusCode::INTERNAL_SERVER_ERROR)));

        parts.extensions.insert(crate::test_support::build_test_tera());
        let result = PageContext::from_request_parts(&mut parts, &state).await;
        assert!(result.is_ok());
    }
}
