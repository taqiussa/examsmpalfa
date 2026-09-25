use sqlx::MySqlPool;
use std::str::FromStr;
use tera::Tera;

use crate::utils::functions::{LabLabel, TahunOptions};

#[derive(Debug)]
pub struct TestDb {
    pub db_name: String,
    pub db_url: String,
    pub pool: MySqlPool,
    admin_pool: MySqlPool,
}

impl TestDb {
    pub async fn new() -> Self {
        let base_url = std::env::var("TEST_DATABASE_URL")
            .expect("TEST_DATABASE_URL must be set for integration tests");

        let admin_pool = MySqlPool::connect(&base_url)
            .await
            .expect("failed to connect to TEST_DATABASE_URL");

        let db_name = format!("test_{}", uuid::Uuid::new_v4().simple());
        let db_url = {
            let mut url = url::Url::parse(&base_url).expect("invalid TEST_DATABASE_URL");
            url.set_path(&format!("/{}", db_name));
            url.to_string()
        };
        let create_sql = format!("CREATE DATABASE `{}`", db_name);
        sqlx::query(&create_sql)
            .execute(&admin_pool)
            .await
            .expect("failed to create test database");

        let mut opts = sqlx::mysql::MySqlConnectOptions::from_str(&base_url)
            .expect("invalid TEST_DATABASE_URL");
        opts = opts.database(&db_name);

        let pool = sqlx::mysql::MySqlPoolOptions::new()
            .max_connections(5)
            .connect_with(opts)
            .await
            .expect("failed to connect to test database");

        init_schema(&pool).await;

        Self {
            db_name,
            db_url,
            pool,
            admin_pool,
        }
    }

    pub async fn try_new() -> Option<Self> {
        if std::env::var("TEST_DATABASE_URL").is_err() {
            eprintln!("Skipping integration test: TEST_DATABASE_URL not set");
            return None;
        }
        Some(Self::new().await)
    }

    pub async fn teardown(self) {
        let drop_sql = format!("DROP DATABASE IF EXISTS `{}`", self.db_name);
        let _ = sqlx::query(&drop_sql).execute(&self.admin_pool).await;
    }
}

pub fn build_test_tera() -> Tera {
    let mut tera = Tera::new("templates/**/*").expect("Failed to init Tera");
    tera.register_function("tahun_options", TahunOptions);
    tera.register_function("lab_label", LabLabel);
    tera
}

#[derive(Debug, Clone)]
pub struct SeedData {
    pub admin_id: u64,
    pub guru_id: u64,
    pub kelas_id: i64,
    pub nis: String,
    pub tahun: String,
}

pub async fn seed_base_data(pool: &MySqlPool) -> SeedData {
    let tahun = crate::utils::tahun::data_tahun();
    let password = bcrypt::hash("secret123", bcrypt::DEFAULT_COST).unwrap();

    let mut tx = pool.begin().await.unwrap();

    sqlx::query(
        r#"
        INSERT INTO roles (name) VALUES
        ('Admin'), ('Guru'), ('Siswa'), ('Konseling')
        "#,
    )
    .execute(&mut *tx)
    .await
    .unwrap();

    sqlx::query(
        r#"
        INSERT INTO users (name, username, password, created_at, updated_at)
        VALUES ('Admin User', 'admin', ?, NOW(), NOW())
        "#,
    )
    .bind(&password)
    .execute(&mut *tx)
    .await
    .unwrap();
    let admin_id: i64 = sqlx::query_scalar("SELECT CAST(LAST_INSERT_ID() AS SIGNED)")
        .fetch_one(&mut *tx)
        .await
        .unwrap();

    sqlx::query(
        r#"
        INSERT INTO users (name, username, password, created_at, updated_at)
        VALUES ('Guru User', 'guru', ?, NOW(), NOW())
        "#,
    )
    .bind(&password)
    .execute(&mut *tx)
    .await
    .unwrap();
    let guru_id: i64 = sqlx::query_scalar("SELECT CAST(LAST_INSERT_ID() AS SIGNED)")
        .fetch_one(&mut *tx)
        .await
        .unwrap();

    let nis = "12345".to_string();
    sqlx::query(
        r#"
        INSERT INTO users (name, username, password, nis, created_at, updated_at)
        VALUES ('Siswa User', 'siswa', ?, ?, NOW(), NOW())
        "#,
    )
    .bind(&password)
    .bind(&nis)
    .execute(&mut *tx)
    .await
    .unwrap();
    let siswa_id: i64 = sqlx::query_scalar("SELECT CAST(LAST_INSERT_ID() AS SIGNED)")
        .fetch_one(&mut *tx)
        .await
        .unwrap();

    let role_admin_id: i64 =
        sqlx::query_scalar("SELECT CAST(id AS SIGNED) FROM roles WHERE name = 'Admin'")
            .fetch_one(&mut *tx)
            .await
            .unwrap();
    let role_guru_id: i64 =
        sqlx::query_scalar("SELECT CAST(id AS SIGNED) FROM roles WHERE name = 'Guru'")
            .fetch_one(&mut *tx)
            .await
            .unwrap();
    let role_siswa_id: i64 =
        sqlx::query_scalar("SELECT CAST(id AS SIGNED) FROM roles WHERE name = 'Siswa'")
            .fetch_one(&mut *tx)
            .await
            .unwrap();

    sqlx::query("INSERT INTO model_has_roles (role_id, model_id) VALUES (?, ?), (?, ?), (?, ?)")
        .bind(role_admin_id)
        .bind(admin_id)
        .bind(role_guru_id)
        .bind(guru_id)
        .bind(role_siswa_id)
        .bind(siswa_id)
        .execute(&mut *tx)
        .await
        .unwrap();

    sqlx::query("INSERT INTO kelas (nama, tingkat) VALUES ('7A', 7)")
        .execute(&mut *tx)
        .await
        .unwrap();
    let kelas_id: i64 = sqlx::query_scalar("SELECT CAST(LAST_INSERT_ID() AS SIGNED)")
        .fetch_one(&mut *tx)
        .await
        .unwrap();

    sqlx::query(
        r#"
        INSERT INTO siswas (nis, tahun, kelas_id, tingkat)
        VALUES (?, ?, ?, 7)
        "#,
    )
    .bind(&nis)
    .bind(&tahun)
    .bind(kelas_id)
    .execute(&mut *tx)
    .await
    .unwrap();

    sqlx::query(
        r#"
        INSERT INTO biodatas (nis, tempat_lahir, tanggal_lahir, alamat_lengkap, telepon)
        VALUES (?, 'Bandung', '2010-01-01', 'Jalan Mawar', '081234')
        "#,
    )
    .bind(&nis)
    .execute(&mut *tx)
    .await
    .unwrap();

    sqlx::query(
        r#"
        INSERT INTO orang_tuas (nis, nama_ayah, nama_ibu)
        VALUES (?, 'Ayah', 'Ibu')
        "#,
    )
    .bind(&nis)
    .execute(&mut *tx)
    .await
    .unwrap();

    tx.commit().await.unwrap();

    SeedData {
        admin_id: admin_id as u64,
        guru_id: guru_id as u64,
        kelas_id: kelas_id as i64,
        nis,
        tahun,
    }
}

async fn init_schema(pool: &MySqlPool) {
    let schema = r#"
        CREATE TABLE users (
            id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY,
            name VARCHAR(255) NOT NULL,
            username VARCHAR(255),
            password VARCHAR(255) NOT NULL,
            nis BIGINT UNIQUE,
            foto VARCHAR(255),
            created_at DATETIME,
            updated_at DATETIME
        );

        CREATE TABLE roles (
            id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY,
            name VARCHAR(100) NOT NULL
        );

        CREATE TABLE model_has_roles (
            role_id BIGINT UNSIGNED NOT NULL,
            model_id BIGINT UNSIGNED NOT NULL,
            model_type VARCHAR(255) NOT NULL DEFAULT 'App\\Models\\User',
            PRIMARY KEY (role_id, model_id, model_type)
        );

        CREATE TABLE kelas (
            id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY,
            nama VARCHAR(255) NOT NULL,
            tingkat VARCHAR(255) NOT NULL,
            created_at DATETIME,
            updated_at DATETIME
        );

        CREATE TABLE siswas (
            id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY,
            nis BIGINT,
            tahun VARCHAR(20) NOT NULL,
            kelas_id BIGINT UNSIGNED NOT NULL,
            tingkat INT NOT NULL,
            created_at DATETIME,
            updated_at DATETIME,
            FOREIGN KEY (kelas_id) REFERENCES kelas(id)
        );

        CREATE TABLE mata_pelajarans (
            id BIGINT AUTO_INCREMENT PRIMARY KEY,
            nama VARCHAR(255) NOT NULL UNIQUE,
            kelompok VARCHAR(10) NOT NULL DEFAULT 'E'
        );

        CREATE TABLE biodatas (
            nis VARCHAR(50) PRIMARY KEY,
            tempat_lahir VARCHAR(100),
            tanggal_lahir DATE,
            alamat_lengkap VARCHAR(255),
            telepon VARCHAR(50)
        );

        CREATE TABLE orang_tuas (
            nis VARCHAR(50) PRIMARY KEY,
            nama_ayah VARCHAR(100),
            nama_ibu VARCHAR(100)
        );

        CREATE TABLE absensis (
            id BIGINT AUTO_INCREMENT PRIMARY KEY,
            tanggal DATE NOT NULL,
            tahun VARCHAR(20) NOT NULL,
            semester INT NOT NULL,
            jam VARCHAR(20) NOT NULL,
            kelas_id BIGINT NOT NULL,
            nis VARCHAR(50) NOT NULL,
            kehadiran_id BIGINT NOT NULL,
            user_id BIGINT,
            created_at DATETIME,
            updated_at DATETIME
        );
    "#;

    for statement in schema.split(';') {
        let sql = statement.trim();
        if !sql.is_empty() {
            sqlx::query(sql).execute(pool).await.unwrap();
        }
    }
}
