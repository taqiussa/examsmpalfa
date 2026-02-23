-- Migration: create hasil_nilais to store siswa exam results
CREATE TABLE IF NOT EXISTS hasil_nilais (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    nis VARCHAR(20) NOT NULL,
    ujian_id BIGINT NOT NULL,
    mata_pelajaran_id BIGINT NOT NULL,
    tahun VARCHAR(20) NOT NULL,
    total_benar INT NOT NULL DEFAULT 0,
    total_salah INT NOT NULL DEFAULT 0,
    total_pg INT NOT NULL DEFAULT 0,
    total_uraian INT NULL,
    total_nilai INT NOT NULL DEFAULT 0,
    created_at DATETIME NOT NULL DEFAULT NOW(),
    updated_at DATETIME NOT NULL DEFAULT NOW(),
    UNIQUE KEY unique_hasil_nilai (nis, ujian_id, mata_pelajaran_id, tahun),
    KEY idx_hasil_nilais_nis (nis),
    KEY idx_hasil_nilais_ujian (ujian_id),
    KEY idx_hasil_nilais_mapel (mata_pelajaran_id),
    KEY idx_hasil_nilais_tahun (tahun),
    CONSTRAINT fk_hasil_nilais_ujian
        FOREIGN KEY (ujian_id) REFERENCES ujians(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
