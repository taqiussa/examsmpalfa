-- Add exam participant grouping by tahun, lab, sesi, and gelombang.
ALTER TABLE ujian_tokens
    ADD COLUMN IF NOT EXISTS tahun VARCHAR(20) NULL AFTER ujian_id,
    ADD COLUMN IF NOT EXISTS lab_kode CHAR(2) NULL AFTER tahun,
    ADD COLUMN IF NOT EXISTS sesi TINYINT UNSIGNED NULL AFTER lab_kode,
    ADD COLUMN IF NOT EXISTS gelombang TINYINT UNSIGNED NULL AFTER sesi;

ALTER TABLE ujian_pesertas
    ADD COLUMN IF NOT EXISTS tahun VARCHAR(20) NULL AFTER token_id,
    ADD COLUMN IF NOT EXISTS kelas_id BIGINT NULL AFTER tahun,
    ADD COLUMN IF NOT EXISTS lab_kode CHAR(2) NULL AFTER kelas_id,
    ADD COLUMN IF NOT EXISTS sesi TINYINT UNSIGNED NULL AFTER lab_kode,
    ADD COLUMN IF NOT EXISTS gelombang TINYINT UNSIGNED NULL AFTER sesi;

UPDATE ujian_tokens t
JOIN ujians u ON u.id = t.ujian_id
SET t.tahun = COALESCE(t.tahun, u.tahun),
    t.lab_kode = COALESCE(t.lab_kode, '01'),
    t.sesi = COALESCE(t.sesi, 1),
    t.gelombang = COALESCE(t.gelombang, 1);

UPDATE ujian_pesertas p
JOIN ujians u ON u.id = p.ujian_id
LEFT JOIN ujian_tokens t ON t.id = p.token_id
LEFT JOIN siswas s ON s.nis = p.nis AND s.tahun = COALESCE(t.tahun, u.tahun)
SET p.tahun = COALESCE(p.tahun, t.tahun, u.tahun),
    p.kelas_id = COALESCE(p.kelas_id, s.kelas_id),
    p.lab_kode = COALESCE(p.lab_kode, t.lab_kode, '01'),
    p.sesi = COALESCE(p.sesi, t.sesi, 1),
    p.gelombang = COALESCE(p.gelombang, t.gelombang, 1),
    p.updated_at = NOW();

CREATE INDEX idx_ujian_tokens_grouping
    ON ujian_tokens (ujian_id, tahun, lab_kode, sesi, gelombang);

CREATE INDEX idx_ujian_pesertas_grouping
    ON ujian_pesertas (ujian_id, tahun, lab_kode, sesi, gelombang, status);

CREATE INDEX idx_ujian_pesertas_kelas
    ON ujian_pesertas (kelas_id);
