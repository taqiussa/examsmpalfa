use crate::{
    controllers::admin::sinkron_penilaian::{
        sinkron_execute, sinkron_penilaian_page, sinkron_preview, sinkron_reference_options,
        sinkron_ujian_options,
    },
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
        .route("/sinkron-penilaian", get(sinkron_penilaian_page))
        .route(
            "/sinkron-penilaian/ujian-options",
            get(sinkron_ujian_options),
        )
        .route(
            "/sinkron-penilaian/reference-options",
            get(sinkron_reference_options),
        )
        .route("/sinkron-penilaian/preview", get(sinkron_preview))
        .route("/sinkron-penilaian/sync", post(sinkron_execute))
        .route_layer(middleware::from_fn(role_middleware))
        .layer(Extension(AllowedRoles(vec![Role::Admin])))
}

async fn dummy_admin_page() -> &'static str {
    "Admin only"
}
