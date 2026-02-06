use axum::response::{IntoResponse, Redirect};

pub async fn root_page() -> impl IntoResponse {
    Redirect::to("/dashboard")
}

#[cfg(test)]
mod tests {
    use super::root_page;
    use axum::response::IntoResponse;

    #[tokio::test]
    async fn root_page_redirects() {
        let response = root_page().await.into_response();
        assert_eq!(response.status(), axum::http::StatusCode::SEE_OTHER);
    }
}
