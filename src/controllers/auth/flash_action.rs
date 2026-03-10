use axum::{Json, extract::Query, http::StatusCode, response::IntoResponse};
use serde::Serialize;

#[derive(Serialize)]
struct FlashResponse {
    status: String,
    message: String,
}

#[derive(serde::Deserialize)]
pub(crate) struct FlashParams {
    #[serde(rename = "type")]
    kind: Option<String>,
    message: Option<String>,
}

pub async fn flash_action(Query(params): Query<FlashParams>) -> impl IntoResponse {
    let kind = params
        .kind
        .as_deref()
        .map(|v| v.to_lowercase())
        .unwrap_or_else(|| "success".to_string());

    let status = match kind.as_str() {
        "success" | "warning" | "error" | "info" => kind.clone(),
        _ => "success".to_string(),
    };

    let message = params.message.unwrap_or_else(|| {
        match status.as_str() {
            "success" => "Sukses memunculkan flash",
            "warning" => "Peringatan berhasil ditampilkan",
            "error" => "Terjadi kesalahan saat memunculkan flash",
            "info" => "Info berhasil ditampilkan",
            _ => "Sukses memunculkan flash",
        }
        .to_string()
    });

    (StatusCode::OK, Json(FlashResponse { status, message }))
}

#[cfg(test)]
mod tests {
    use super::{FlashParams, flash_action};
    use axum::extract::Query;
    use axum::response::IntoResponse;

    #[tokio::test]
    async fn flash_action_defaults_to_success() {
        let params = FlashParams {
            kind: Some("unknown".into()),
            message: None,
        };
        let response = flash_action(Query(params)).await.into_response();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
    }
}
