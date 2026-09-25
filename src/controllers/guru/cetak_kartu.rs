use axum::{Extension, extract::Query, http::StatusCode, response::Html};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, MySqlPool};
use tera::Context;

use crate::utils::{
    list_kelas::{ListKelas, list_kelas},
    page_context::PageContext,
    render::render,
    tahun::data_tahun,
};

const CARDS_PER_PAGE: usize = 10;

#[derive(Serialize)]
struct CetakKartuPageData {
    tahun: String,
    kelas_id: u64,
    require_kelas: bool,
    list_kelas: Vec<ListKelas>,
}

#[derive(Debug, Deserialize)]
pub struct CetakKartuFilter {
    tahun: String,
    kelas_id: u64,
    nama_ujian: String,
}

#[derive(Clone, Debug, Serialize, FromRow)]
struct KartuPeserta {
    nis: String,
    nama: String,
    kelas: String,
    sesi: Option<i32>,
    gelombang: Option<i32>,
}

#[derive(Serialize)]
struct CetakKartuPrintData {
    tahun: String,
    nama_ujian: String,
    kelas: String,
    pages: Vec<Vec<KartuPeserta>>,
    total_peserta: usize,
}

pub async fn cetak_kartu_page(
    ctx: PageContext,
    Extension(db): Extension<MySqlPool>,
) -> Html<String> {
    let data = CetakKartuPageData {
        tahun: data_tahun(),
        kelas_id: 0,
        require_kelas: true,
        list_kelas: list_kelas(&db).await.unwrap_or_default(),
    };

    render(&ctx, "guru/cetak_kartu/index.html", "Cetak Kartu", data)
}

pub async fn cetak_kartu_print(
    ctx: PageContext,
    Query(filter): Query<CetakKartuFilter>,
    Extension(db): Extension<MySqlPool>,
) -> Result<Html<String>, StatusCode> {
    let tahun = filter.tahun.trim();
    let nama_ujian = filter.nama_ujian.trim();

    if tahun.is_empty() || nama_ujian.is_empty() || filter.kelas_id == 0 {
        return Err(StatusCode::BAD_REQUEST);
    }

    let kelas = sqlx::query_scalar::<_, String>("SELECT nama FROM kelas WHERE id = ?")
        .bind(filter.kelas_id)
        .fetch_optional(&db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    // A student remains printable even when they have not been assigned to an
    // exam session. The grouped participant join also prevents duplicate cards.
    let peserta = sqlx::query_as::<_, KartuPeserta>(
        r#"
        SELECT
            CAST(s.nis AS CHAR) AS nis,
            CAST(COALESCE(u.name, CONCAT('NIS ', s.nis)) AS CHAR) AS nama,
            CAST(k.nama AS CHAR) AS kelas,
            CAST(p.sesi AS SIGNED) AS sesi,
            CAST(p.gelombang AS SIGNED) AS gelombang
        FROM siswas s
        INNER JOIN kelas k ON k.id = s.kelas_id
        LEFT JOIN users u ON u.nis = s.nis
        LEFT JOIN ujian_pesertas p ON p.id = (
            SELECT p2.id
            FROM ujian_pesertas p2
            WHERE p2.tahun = s.tahun AND p2.nis = s.nis
            ORDER BY p2.id DESC
            LIMIT 1
        )
        WHERE s.tahun = ? AND s.kelas_id = ?
        ORDER BY u.name ASC, s.nis ASC
        "#,
    )
    .bind(tahun)
    .bind(filter.kelas_id)
    .fetch_all(&db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let total_peserta = peserta.len();
    let pages = paginate_cards(peserta);
    let data = CetakKartuPrintData {
        tahun: tahun.to_owned(),
        nama_ujian: nama_ujian.to_owned(),
        kelas,
        pages,
        total_peserta,
    };

    let tera_ctx = Context::from_serialize(data).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let html = ctx
        .tera
        .render("guru/cetak_kartu/print.html", &tera_ctx)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Html(html))
}

fn paginate_cards(rows: Vec<KartuPeserta>) -> Vec<Vec<KartuPeserta>> {
    rows.chunks(CARDS_PER_PAGE)
        .map(|chunk| chunk.to_vec())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{KartuPeserta, paginate_cards};

    #[test]
    fn print_pages_contain_at_most_ten_cards() {
        let rows = (1..=21)
            .map(|number| KartuPeserta {
                nis: number.to_string(),
                nama: format!("Peserta {number}"),
                kelas: "7A".into(),
                sesi: None,
                gelombang: None,
            })
            .collect();

        let pages = paginate_cards(rows);
        assert_eq!(
            pages.iter().map(Vec::len).collect::<Vec<_>>(),
            vec![10, 10, 1]
        );
    }
}
