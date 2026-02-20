-- Migration: Create ujian_soals if missing (safe)
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
