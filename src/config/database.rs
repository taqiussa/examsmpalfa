use sqlx::mysql::{MySqlPool, MySqlPoolOptions};

pub async fn connect() -> MySqlPool {
    // Ambil DATABASE_URL dari environment variable
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must set");

    // Coba koneksi ke database
    match MySqlPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await
    {
        Ok(pool) => {
            println!("Database Connected Successfully!");
            pool
        }
        Err(err) => {
            eprintln!("Failed to Connect to Database: {err:?}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::connect;
    use serial_test::serial;

    #[tokio::test]
    #[serial]
    async fn connect_uses_database_url_env() {
        let Some(test_db) = crate::test_support::TestDb::try_new().await else {
            return;
        };
        unsafe { std::env::set_var("DATABASE_URL", &test_db.db_url) };

        let pool = connect().await;
        let result: i32 = sqlx::query_scalar!("SELECT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(result, 1);

        unsafe { std::env::remove_var("DATABASE_URL") };
        test_db.teardown().await;
    }
}
