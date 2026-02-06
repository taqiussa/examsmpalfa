use axum::{Extension, Router, middleware, routing::get};

use crate::{
    controllers::auth::dashboard_page::dashboard_page,
    middlewares::role_middleware::{AllowedRoles, role_middleware},
    models::role::Role,
};

pub fn siswa_routes() -> Router {
    Router::new()
        .route("/siswa", get(dashboard_page))
        // 🔒 ROLE HARUS ROUTE_LAYER
        .route_layer(middleware::from_fn(role_middleware))
        .layer(Extension(AllowedRoles(vec![Role::Siswa])))
}
