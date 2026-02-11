use axum::{
    Extension, Router, middleware,
    routing::{get, post},
};

use crate::{
    controllers::siswa::ujian_siswa::{
        ujian_gate, ujian_konfirmasi_token, ujian_session_page, ujian_simpan_jawaban, ujian_submit,
    },
    middlewares::role_middleware::{AllowedRoles, role_middleware},
    models::role::Role,
};

pub fn siswa_routes() -> Router {
    Router::new()
        .route("/siswa", get(ujian_gate))
        .route("/siswa/ujian", get(ujian_gate))
        .route("/siswa/ujian/konfirmasi", post(ujian_konfirmasi_token))
        .route("/siswa/ujian/{id}", get(ujian_session_page))
        .route("/siswa/ujian/{id}/jawab", post(ujian_simpan_jawaban))
        .route("/siswa/ujian/{id}/submit", post(ujian_submit))
        // 🔒 ROLE HARUS ROUTE_LAYER
        .route_layer(middleware::from_fn(role_middleware))
        .layer(Extension(AllowedRoles(vec![Role::Siswa])))
}
