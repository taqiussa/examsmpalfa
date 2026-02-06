use axum::{
    Router, middleware,
    routing::{get, post},
};

use crate::{
    controllers::auth::{login_action::login_action, login_page::login_page},
    middlewares::guest_middleware::guest_middleware,
};

pub fn public_routes() -> Router {
    Router::new()
        .route(
            "/login",
            get(login_page).layer(middleware::from_fn(guest_middleware)),
        )
        .route("/login", post(login_action))
}
