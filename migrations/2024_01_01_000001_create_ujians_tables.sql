-- Migration: Create Ujian Tables
-- Created for: Guru role to create and manage online exams
-- Supports: Multiple choice questions with rich text

-- ======================
-- TABLE: ujians
-- ======================
DROP TRIGGER IF EXISTS after_ujian_soal_insert;
DROP TRIGGER IF EXISTS after_ujian_soal_delete;

DROP TABLE IF EXISTS ujian_soals;
DROP TABLE IF EXISTS soals;
DROP TABLE IF EXISTS ujians;

CREATE TABLE IF NOT EXISTS ujians (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    mata_pelajaran_id BIGINT UNSIGNED NOT NULL,
    jurusan ENUM('UMUM','PBS','TKR','TKJ') NOT NULL DEFAULT 'UMUM',
    title VARCHAR(255) NOT NULL,
    description TEXT,
    tanggal DATE NOT NULL,
    waktu_menit INT NOT NULL DEFAULT 60,
    total_soal INT DEFAULT 0,
    is_active BOOLEAN DEFAULT TRUE,
    created_by BIGINT,
    created_at DATETIME,
    updated_at DATETIME,
    CONSTRAINT fk_ujians_mapel
        FOREIGN KEY (mata_pelajaran_id) REFERENCES mata_pelajarans(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ======================
-- TABLE: soals
-- ======================
CREATE TABLE IF NOT EXISTS soals (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    pertanyaan TEXT NOT NULL,
    opsi_a TEXT,
    opsi_b TEXT,
    opsi_c TEXT,
    opsi_d TEXT,
    kunci_jawaban ENUM('a', 'b', 'c', 'd') NOT NULL,
    bobot_nilai INT DEFAULT 1,
    kategori VARCHAR(100),
    created_at DATETIME,
    updated_at DATETIME
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ======================
-- TABLE: ujian_soals
-- ======================
CREATE TABLE IF NOT EXISTS ujian_soals (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    ujian_id BIGINT NOT NULL,
    soal_id BIGINT NOT NULL,
    urutan INT DEFAULT 1,
    created_at DATETIME,
    FOREIGN KEY (ujian_id) REFERENCES ujians(id) ON DELETE CASCADE,
    FOREIGN KEY (soal_id) REFERENCES soals(id) ON DELETE CASCADE,
    UNIQUE KEY unique_ujian_soal (ujian_id, soal_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ======================
-- INDEXES
-- ======================
CREATE INDEX idx_ujians_tanggal ON ujians(tanggal);
CREATE INDEX idx_ujians_active ON ujians(is_active);
CREATE INDEX idx_ujians_mapel ON ujians(mata_pelajaran_id);
CREATE INDEX idx_ujians_jurusan ON ujians(jurusan);
CREATE INDEX idx_soals_kategori ON soals(kategori);
CREATE INDEX idx_ujian_soals_ujian ON ujian_soals(ujian_id);
CREATE INDEX idx_ujian_soals_soal ON ujian_soals(soal_id);

-- ======================
-- TRIGGERS
-- ======================

DROP TRIGGER IF EXISTS after_ujian_soal_insert;
CREATE TRIGGER after_ujian_soal_insert
AFTER INSERT ON ujian_soals
FOR EACH ROW
UPDATE ujians
SET total_soal = (
    SELECT COUNT(*) FROM ujian_soals WHERE ujian_id = NEW.ujian_id
),
updated_at = NOW()
WHERE id = NEW.ujian_id;

DROP TRIGGER IF EXISTS after_ujian_soal_delete;
CREATE TRIGGER after_ujian_soal_delete
AFTER DELETE ON ujian_soals
FOR EACH ROW
UPDATE ujians
SET total_soal = (
    SELECT COUNT(*) FROM ujian_soals WHERE ujian_id = OLD.ujian_id
),
updated_at = NOW()
WHERE id = OLD.ujian_id;
