use crate::{
    controllers::admin::tambah_pengguna::{
        hapus_pengguna_action, pengguna_table, tambah_pengguna_action, tambah_pengguna_page,
    },
    middlewares::role_middleware::{AllowedRoles, role_middleware},
    models::role::Role,
};
use axum::{
    Extension, Router, middleware,
    routing::{get, post},
};

pub fn admin_routes() -> Router {
    Router::new()
        .route("/admin-siswa", get(dummy_admin_page))
        .route("/tambah-pengguna", get(tambah_pengguna_page))
        .route("/tambah-pengguna", post(tambah_pengguna_action))
        .route("/tambah-pengguna/table", get(pengguna_table))
        .route("/tambah-pengguna/delete", post(hapus_pengguna_action))
        .route_layer(middleware::from_fn(role_middleware))
        .layer(Extension(AllowedRoles(vec![Role::Admin])))
}

async fn dummy_admin_page() -> &'static str {
    "Admin only"
}
