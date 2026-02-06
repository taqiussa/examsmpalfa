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
            soal_create, soal_delete, soal_store, ujian_create, ujian_delete, ujian_index,
            ujian_show, ujian_store, ujian_table,
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
        .route("/ujian/store", post(ujian_store))
        .route("/ujian/{id}", get(ujian_show))
        .route("/ujian/{id}", delete(ujian_delete))
        .route("/ujian/{id}/soal/create", get(soal_create))
        .route("/ujian/{id}/soal/store", post(soal_store))
        .route("/ujian/{id}/soal/{soal_id}", delete(soal_delete))
        .route_layer(middleware::from_fn(role_middleware))
        .layer(Extension(AllowedRoles(vec![Role::Guru, Role::Konseling])))
}
