use axum::{Extension, Router, middleware, routing::{get, post}};

use crate::{
    controllers::guru::{
        absensi_kelas::{
            absensi_kelas, absensi_kelas_mark_all, absensi_kelas_table, absensi_kelas_update,
        },
        biodata_siswa::{biodata_siswa, biodata_siswa_table},
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
        .route_layer(middleware::from_fn(role_middleware))
        .layer(Extension(AllowedRoles(vec![Role::Guru, Role::Konseling])))
}
