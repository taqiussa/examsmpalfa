use axum::{
    Extension, Router,
    extract::DefaultBodyLimit,
    middleware,
    routing::{delete, get, post},
};

use crate::{
    controllers::admin::upload_peserta::{
        download_draft_peserta, upload_peserta_action, upload_peserta_page,
    },
    controllers::guru::{
        absensi_kelas::{
            absensi_kelas, absensi_kelas_mark_all, absensi_kelas_table, absensi_kelas_update,
        },
        cetak_kartu::{cetak_kartu_page, cetak_kartu_print},
        data_kelas::{
            data_kelas_create, data_kelas_destroy, data_kelas_edit, data_kelas_index,
            data_kelas_store, data_kelas_update,
        },
        data_peserta::{data_peserta, data_peserta_table},
        mata_pelajaran::{
            mata_pelajaran_create, mata_pelajaran_edit, mata_pelajaran_index, mata_pelajaran_store,
            mata_pelajaran_update,
        },
        ujian::{
            nilai_kelas_mapel_export, nilai_kelas_mapel_page, nilai_kelas_mapel_table, soal_create,
            soal_delete, soal_image_upload, soal_store, status_peserta_page, status_peserta_table,
            status_peserta_toggle, ujian_create, ujian_delete, ujian_delete_token,
            ujian_generate_token, ujian_index, ujian_progress, ujian_progress_active_panel,
            ujian_show, ujian_store, ujian_table, ujian_toggle_active, ujian_toggle_token,
            ujian_uraian_review, ujian_uraian_score, ujian_uraian_table,
        },
        upload_data_siswa::{
            download_draft_data_siswa, import_data_siswa, upload_data_siswa_page,
            upload_data_siswa_preview,
        },
    },
    middlewares::role_middleware::{AllowedRoles, role_middleware},
    models::role::Role,
};

const UPLOAD_DATA_SISWA_BODY_LIMIT: usize = 6 * 1024 * 1024;

pub fn guru_only_routes() -> Router {
    Router::new()
        .route("/cetak-kartu", get(cetak_kartu_page))
        .route("/cetak-kartu/print", get(cetak_kartu_print))
        .route("/upload-data-siswa", get(upload_data_siswa_page))
        .route("/upload-data-siswa/draft", get(download_draft_data_siswa))
        .route("/upload-peserta", get(upload_peserta_page))
        .route("/upload-peserta", post(upload_peserta_action))
        .route("/upload-peserta/draft", get(download_draft_peserta))
        .route("/data-kelas", get(data_kelas_index))
        .route("/data-kelas", post(data_kelas_store))
        .route("/data-kelas/create", get(data_kelas_create))
        .route("/data-kelas/{id}/edit", get(data_kelas_edit))
        .route("/data-kelas/{id}", post(data_kelas_update))
        .route("/data-kelas/{id}/delete", post(data_kelas_destroy))
        .route(
            "/upload-data-siswa/preview",
            post(upload_data_siswa_preview)
                .layer(DefaultBodyLimit::max(UPLOAD_DATA_SISWA_BODY_LIMIT)),
        )
        .route(
            "/upload-data-siswa/import",
            post(import_data_siswa).layer(DefaultBodyLimit::max(UPLOAD_DATA_SISWA_BODY_LIMIT)),
        )
        .route_layer(middleware::from_fn(role_middleware))
        .layer(Extension(AllowedRoles(vec![Role::Guru])))
}

pub fn guru_routes() -> Router {
    Router::new()
        .route("/absensi-kelas", get(absensi_kelas))
        .route("/absensi-kelas/table", get(absensi_kelas_table))
        .route("/absensi-kelas/mark-all", post(absensi_kelas_mark_all))
        .route("/absensi-kelas/update", post(absensi_kelas_update))
        .route("/data-peserta", get(data_peserta))
        .route("/data-peserta/table", get(data_peserta_table))
        .route("/mata-pelajaran", get(mata_pelajaran_index))
        .route("/mata-pelajaran", post(mata_pelajaran_store))
        .route("/mata-pelajaran/create", get(mata_pelajaran_create))
        .route("/mata-pelajaran/{id}/edit", get(mata_pelajaran_edit))
        .route("/mata-pelajaran/{id}", post(mata_pelajaran_update))
        .route("/ujian", get(ujian_index))
        .route("/ujian/table", get(ujian_table))
        .route("/ujian/create", get(ujian_create))
        .route("/progress-ujian", get(ujian_progress))
        .route(
            "/progress-ujian/active-panel",
            get(ujian_progress_active_panel),
        )
        .route("/status-peserta", get(status_peserta_page))
        .route("/status-peserta/table", get(status_peserta_table))
        .route(
            "/status-peserta/{peserta_id}/toggle",
            post(status_peserta_toggle),
        )
        .route("/review-uraian", get(ujian_uraian_review))
        .route("/review-uraian/table", get(ujian_uraian_table))
        .route("/review-uraian/score", post(ujian_uraian_score))
        .route("/hasil-nilai", get(nilai_kelas_mapel_page))
        .route("/hasil-nilai/table", get(nilai_kelas_mapel_table))
        .route("/hasil-nilai/export", get(nilai_kelas_mapel_export))
        .route("/ujian/store", post(ujian_store))
        .route("/ujian/{id}", get(ujian_show))
        .route("/ujian/{id}", delete(ujian_delete))
        .route("/ujian/{id}/toggle-active", post(ujian_toggle_active))
        .route("/ujian/{id}/token", post(ujian_generate_token))
        .route(
            "/ujian/{id}/token/{token_id}/toggle",
            post(ujian_toggle_token),
        )
        .route("/ujian/{id}/token/{token_id}", delete(ujian_delete_token))
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
