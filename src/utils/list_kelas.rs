use serde::Serialize;
use sqlx::MySqlPool;

#[derive(Debug, Serialize)]
pub struct ListKelas {
    pub id: u64,
    pub nama: String,
}

pub async fn list_kelas(pool: &MySqlPool) -> Result<Vec<ListKelas>, sqlx::Error> {
    let rows = sqlx::query_as!(
        ListKelas,
        r#"
        SELECT
            id,
            nama
        FROM kelas
        ORDER BY tingkat, nama
        "#
    )
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::list_kelas;
    use serial_test::serial;

    #[tokio::test]
    #[serial]
    async fn list_kelas_returns_sorted_rows() {
        let Some(test_db) = crate::test_support::TestDb::try_new().await else {
            return;
        };
        crate::test_support::seed_base_data(&test_db.pool).await;

        let rows = list_kelas(&test_db.pool).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].nama, "7A");

        test_db.teardown().await;
    }
}
