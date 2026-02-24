-- Migration: Support Pilihan Ganda Kompleks and Benar/Salah scoring
-- 1) Change soals.kunci_jawaban to VARCHAR to allow multiple keys (e.g. 'a,c') or 'benar'/'salah'
-- 2) Change ujian_jawabans.pilihan to VARCHAR to store multiple selections
-- 3) Allow decimal bobot_nilai in soals and ujian_jawabans; totals to DECIMAL

ALTER TABLE soals
    MODIFY kunci_jawaban VARCHAR(255) NULL,
    MODIFY bobot_nilai DECIMAL(5,2) DEFAULT 1;

ALTER TABLE ujian_jawabans
    MODIFY pilihan VARCHAR(50) NULL,
    MODIFY bobot_nilai DECIMAL(5,2) NOT NULL DEFAULT 0;

ALTER TABLE ujian_pesertas
    MODIFY total_nilai DECIMAL(7,2) NULL;

ALTER TABLE hasil_nilais
    MODIFY total_pg DECIMAL(7,2) NOT NULL DEFAULT 0,
    MODIFY total_nilai DECIMAL(7,2) NOT NULL DEFAULT 0;

-- Down (manual): revert column types with care; backup data before rollback.
