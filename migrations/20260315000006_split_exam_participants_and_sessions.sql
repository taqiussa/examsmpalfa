-- Split participant registry and exam sessions.

SET @schema := DATABASE();

SET @has_old_pesertas := (
    SELECT COUNT(*)
    FROM information_schema.tables
    WHERE table_schema = @schema AND table_name = 'ujian_pesertas'
);
SET @has_pengerjaans := (
    SELECT COUNT(*)
    FROM information_schema.tables
    WHERE table_schema = @schema AND table_name = 'ujian_pengerjaans'
);

SET @rename_sql := IF(
    @has_old_pesertas = 1 AND @has_pengerjaans = 0,
    'RENAME TABLE ujian_pesertas TO ujian_pengerjaans',
    'SELECT 1'
);
PREPARE stmt FROM @rename_sql;
EXECUTE stmt;
DEALLOCATE PREPARE stmt;

CREATE TABLE IF NOT EXISTS ujian_pesertas (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    ujian_id BIGINT NOT NULL,
    tahun VARCHAR(20) NOT NULL,
    nis VARCHAR(50) NOT NULL,
    kelas_id BIGINT NULL,
    lab_kode CHAR(2) NOT NULL,
    sesi TINYINT UNSIGNED NOT NULL,
    gelombang TINYINT UNSIGNED NOT NULL,
    created_at DATETIME NOT NULL DEFAULT NOW(),
    updated_at DATETIME NOT NULL DEFAULT NOW(),
    UNIQUE KEY unique_exam_participant (ujian_id, nis),
    KEY idx_ujian_pesertas_exam_group (ujian_id, tahun, lab_kode, sesi, gelombang),
    KEY idx_ujian_pesertas_nis (nis),
    KEY idx_ujian_pesertas_kelas (kelas_id),
    CONSTRAINT fk_ujian_pesertas_exam_registry
        FOREIGN KEY (ujian_id) REFERENCES ujians(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

ALTER TABLE ujian_pengerjaans
    MODIFY nis VARCHAR(50) NOT NULL;

INSERT IGNORE INTO ujian_pesertas
    (ujian_id, tahun, nis, kelas_id, lab_kode, sesi, gelombang, created_at, updated_at)
SELECT
    ujian_id,
    COALESCE(tahun, ''),
    nis,
    kelas_id,
    COALESCE(lab_kode, '01'),
    COALESCE(sesi, 1),
    COALESCE(gelombang, 1),
    COALESCE(created_at, NOW()),
    COALESCE(updated_at, NOW())
FROM ujian_pengerjaans;

CREATE INDEX idx_ujian_pengerjaans_status
    ON ujian_pengerjaans (ujian_id, status);
