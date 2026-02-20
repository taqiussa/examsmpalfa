-- Migration: add tahun to ujians
ALTER TABLE ujians
    ADD COLUMN tahun VARCHAR(20) NOT NULL DEFAULT '' AFTER mata_pelajaran_id;

CREATE INDEX idx_ujians_tahun ON ujians(tahun);
