SET @schema := DATABASE();

CREATE TABLE IF NOT EXISTS ujian_pesertas_global_tmp (
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
    KEY idx_ujian_pesertas_global_group (tahun, lab_kode, sesi, gelombang),
    KEY idx_ujian_pesertas_global_nis (nis),
    KEY idx_ujian_pesertas_global_kelas (kelas_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

INSERT INTO ujian_pesertas_global_tmp
    (tahun, nis, kelas_id, lab_kode, sesi, gelombang, created_at, updated_at)
SELECT
    COALESCE(NULLIF(tahun, ''), '') as tahun,
    nis,
    MAX(kelas_id) as kelas_id,
    COALESCE(NULLIF(lab_kode, ''), '01') as lab_kode,
    COALESCE(sesi, 1) as sesi,
    COALESCE(gelombang, 1) as gelombang,
    MIN(COALESCE(created_at, NOW())) as created_at,
    MAX(COALESCE(updated_at, NOW())) as updated_at
FROM ujian_pesertas
GROUP BY
    COALESCE(NULLIF(tahun, ''), ''),
    nis,
    COALESCE(NULLIF(lab_kode, ''), '01'),
    COALESCE(sesi, 1),
    COALESCE(gelombang, 1)
ON DUPLICATE KEY UPDATE
    kelas_id = VALUES(kelas_id),
    updated_at = GREATEST(ujian_pesertas_global_tmp.updated_at, VALUES(updated_at));

DROP TABLE ujian_pesertas;
RENAME TABLE ujian_pesertas_global_tmp TO ujian_pesertas;
