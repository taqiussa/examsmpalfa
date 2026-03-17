-- Normalize ujian_pesertas into a global participant registry.
-- Participants belong to a yearly session/group registry, while per-exam work
-- lives in ujian_pengerjaans.

SET @schema := DATABASE();

CREATE TABLE IF NOT EXISTS ujian_pesertas_registry_tmp (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    tahun VARCHAR(20) NOT NULL,
    nis VARCHAR(50) NOT NULL,
    kelas_id BIGINT NULL,
    lab_kode CHAR(2) NOT NULL,
    sesi TINYINT UNSIGNED NOT NULL,
    gelombang TINYINT UNSIGNED NOT NULL,
    created_at DATETIME NOT NULL DEFAULT NOW(),
    updated_at DATETIME NOT NULL DEFAULT NOW(),
    UNIQUE KEY unique_global_exam_participant (tahun, nis, lab_kode, sesi, gelombang),
    KEY idx_ujian_pesertas_group (tahun, lab_kode, sesi, gelombang),
    KEY idx_ujian_pesertas_nis (nis),
    KEY idx_ujian_pesertas_kelas (kelas_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

SET @has_ujian_pesertas := (
    SELECT COUNT(*)
    FROM information_schema.tables
    WHERE table_schema = @schema
      AND table_name = 'ujian_pesertas'
);

SET @ujian_pesertas_has_ujian_id := (
    SELECT COUNT(*)
    FROM information_schema.columns
    WHERE table_schema = @schema
      AND table_name = 'ujian_pesertas'
      AND column_name = 'ujian_id'
);

SET @copy_sql := IF(
    @has_ujian_pesertas = 0,
    'SELECT 1',
    IF(
        @ujian_pesertas_has_ujian_id > 0,
        'INSERT INTO ujian_pesertas_registry_tmp
            (tahun, nis, kelas_id, lab_kode, sesi, gelombang, created_at, updated_at)
         SELECT
            COALESCE(NULLIF(tahun, ''''), '''') AS tahun,
            nis,
            MAX(kelas_id) AS kelas_id,
            COALESCE(NULLIF(lab_kode, ''''), ''01'') AS lab_kode,
            COALESCE(sesi, 1) AS sesi,
            COALESCE(gelombang, 1) AS gelombang,
            MIN(COALESCE(created_at, NOW())) AS created_at,
            MAX(COALESCE(updated_at, NOW())) AS updated_at
         FROM ujian_pesertas
         GROUP BY
            COALESCE(NULLIF(tahun, ''''), ''''),
            nis,
            COALESCE(NULLIF(lab_kode, ''''), ''01''),
            COALESCE(sesi, 1),
            COALESCE(gelombang, 1)
         ON DUPLICATE KEY UPDATE
            kelas_id = VALUES(kelas_id),
            updated_at = GREATEST(ujian_pesertas_registry_tmp.updated_at, VALUES(updated_at))',
        'INSERT INTO ujian_pesertas_registry_tmp
            (tahun, nis, kelas_id, lab_kode, sesi, gelombang, created_at, updated_at)
         SELECT
            COALESCE(NULLIF(tahun, ''''), '''') AS tahun,
            nis,
            kelas_id,
            COALESCE(NULLIF(lab_kode, ''''), ''01'') AS lab_kode,
            COALESCE(sesi, 1) AS sesi,
            COALESCE(gelombang, 1) AS gelombang,
            COALESCE(created_at, NOW()) AS created_at,
            COALESCE(updated_at, NOW()) AS updated_at
         FROM ujian_pesertas
         ON DUPLICATE KEY UPDATE
            kelas_id = VALUES(kelas_id),
            updated_at = GREATEST(ujian_pesertas_registry_tmp.updated_at, VALUES(updated_at))'
    )
);

PREPARE stmt FROM @copy_sql;
EXECUTE stmt;
DEALLOCATE PREPARE stmt;

SET @swap_sql := IF(
    @has_ujian_pesertas = 0,
    'RENAME TABLE ujian_pesertas_registry_tmp TO ujian_pesertas',
    'RENAME TABLE ujian_pesertas TO ujian_pesertas_legacy_tmp, ujian_pesertas_registry_tmp TO ujian_pesertas'
);

PREPARE stmt FROM @swap_sql;
EXECUTE stmt;
DEALLOCATE PREPARE stmt;

SET @drop_legacy_sql := IF(
    @has_ujian_pesertas = 0,
    'SELECT 1',
    'DROP TABLE ujian_pesertas_legacy_tmp'
);

PREPARE stmt FROM @drop_legacy_sql;
EXECUTE stmt;
DEALLOCATE PREPARE stmt;
