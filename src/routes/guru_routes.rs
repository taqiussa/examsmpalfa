use axum::{
    Extension, Router, middleware,
    routing::{delete, get, post},
};

use crate::{
    controllers::guru::{
        absensi_kelas::{
            absensi_kelas, absensi_kelas_mark_all, absensi_kelas_table, absensi_kelas_update,
        },
        biodata_siswa::{biodata_siswa, biodata_siswa_table},
        ujian::{
            soal_create, soal_delete, soal_image_upload, soal_store, ujian_create, ujian_delete,
            ujian_index, ujian_generate_token, ujian_progress, ujian_progress_active_panel,
            ujian_show, ujian_store, ujian_table, ujian_toggle_active, ujian_toggle_token,
            ujian_uraian_review, ujian_uraian_score, ujian_uraian_table,
        },
    },
    middlewares::role_middleware::{AllowedRoles, role_middleware},
    models::role::Role,
};

pub fn guru_routes() -> Router {
    Router::new()
        .route("/absensi-kelas", get(absensi_kelas))
        .route("/absensi-kelas/table", get(absensi_kelas_table))
        .route("/absensi-kelas/mark-all", post(absensi_kelas_mark_all))
        .route("/absensi-kelas/update", post(absensi_kelas_update))
        .route("/biodata-siswa", get(biodata_siswa))
        .route("/biodata-siswa/table", get(biodata_siswa_table))
        .route("/ujian", get(ujian_index))
        .route("/ujian/table", get(ujian_table))
        .route("/ujian/create", get(ujian_create))
        .route("/progress-ujian", get(ujian_progress))
        .route("/progress-ujian/active-panel", get(ujian_progress_active_panel))
        .route("/ujian/uraian", get(ujian_uraian_review))
        .route("/ujian/uraian/table", get(ujian_uraian_table))
        .route("/ujian/uraian/score", post(ujian_uraian_score))
        .route("/ujian/store", post(ujian_store))
        .route("/ujian/{id}", get(ujian_show))
        .route("/ujian/{id}", delete(ujian_delete))
        .route("/ujian/{id}/toggle-active", post(ujian_toggle_active))
        .route("/ujian/{id}/token", post(ujian_generate_token))
        .route("/ujian/{id}/token/{token_id}/toggle", post(ujian_toggle_token))
        .route("/ujian/{id}/soal/create", get(soal_create))
        .route("/ujian/{id}/soal/store", post(soal_store))
        .route("/ujian/{id}/upload-image", post(soal_image_upload))
        .route("/ujian/{id}/soal/{soal_id}", delete(soal_delete))
        .route_layer(middleware::from_fn(role_middleware))
        .layer(Extension(AllowedRoles(vec![
            Role::Admin,
            Role::Guru,
            Role::Konseling,
        ])))
}
