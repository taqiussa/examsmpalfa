use axum::{Router, routing::{get, post}};

use crate::controllers::auth::{
    dashboard_page::dashboard_page, flash_action::flash_action, logout_action::logout_action,
    root_page::root_page,
};

pub fn auth_routes() -> Router {
    Router::new()
        .route("/", get(root_page))
        .route("/dashboard", get(dashboard_page))
        .route("/dashboard/flash", post(flash_action))
        .route("/logout", get(logout_action))
}
