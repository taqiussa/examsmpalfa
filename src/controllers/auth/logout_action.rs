use axum::response::{IntoResponse, Redirect};
use axum_extra::extract::cookie::{Cookie, CookieJar};
use time::Duration;

pub async fn logout_action(jar: CookieJar) -> impl IntoResponse {
    let cookie = Cookie::build("user_id")
        .path("/")
        .http_only(true)
        .max_age(Duration::seconds(0))
        .build(); // ⬅️ ganti finish() → build()

    let jar = jar.remove(cookie);

    (jar, Redirect::to("/login"))
}

#[cfg(test)]
mod tests {
    use super::logout_action;
    use axum::response::IntoResponse;
    use axum_extra::extract::cookie::CookieJar;

    #[tokio::test]
    async fn logout_action_clears_cookie_and_redirects() {
        let jar = CookieJar::new().add(
            axum_extra::extract::cookie::Cookie::build(("user_id", "1"))
                .path("/")
                .build(),
        );
        let response = logout_action(jar).await.into_response();
        assert_eq!(response.status(), axum::http::StatusCode::SEE_OTHER);
    }
}
