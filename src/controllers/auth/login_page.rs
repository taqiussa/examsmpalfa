use axum::{Extension, response::Html};
use tera::{Context, Tera};

use crate::utils::vite;

pub async fn login_page(Extension(tera): Extension<Tera>) -> Html<String> {
    let mut ctx = Context::new();
    ctx.insert("title", "Login");
    ctx.insert("vite_css", &vite::vite_css());
    ctx.insert("vite_js", &vite::vite_js());

    let rendered = tera.render("auth/login.html", &ctx).unwrap();
    Html(rendered)
}

#[cfg(test)]
mod tests {
    use super::login_page;
    use axum::{Extension, response::IntoResponse};

    #[tokio::test]
    async fn login_page_renders() {
        let response = login_page(Extension(crate::test_support::build_test_tera()))
            .await
            .into_response();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
    }
}
