-- Migration: Fix hasil_nilais columns to DECIMAL(5,2) to support decimal scores
-- This ensures all score columns support decimal values (e.g., 0.5 for partial credit)

-- First, normalize existing values to numeric and ensure non-null
UPDATE hasil_nilais SET total_benar = COALESCE(CAST(total_benar AS DECIMAL(5,2)), 0.00);
UPDATE hasil_nilais SET total_salah = COALESCE(CAST(total_salah AS DECIMAL(5,2)), 0.00);
UPDATE hasil_nilais SET total_pg = COALESCE(CAST(total_pg AS DECIMAL(5,2)), 0.00);
UPDATE hasil_nilais SET total_uraian = COALESCE(CAST(total_uraian AS DECIMAL(5,2)), 0.00);
UPDATE hasil_nilais SET total_nilai = COALESCE(CAST(total_nilai AS DECIMAL(5,2)), 0.00);

-- Change column types to DECIMAL(5,2)
ALTER TABLE hasil_nilais
    MODIFY COLUMN total_benar DECIMAL(5,2) NOT NULL DEFAULT 0.00,
    MODIFY COLUMN total_salah DECIMAL(5,2) NOT NULL DEFAULT 0.00,
    MODIFY COLUMN total_pg DECIMAL(5,2) NOT NULL DEFAULT 0.00,
    MODIFY COLUMN total_uraian DECIMAL(5,2) NOT NULL DEFAULT 0.00,
    MODIFY COLUMN total_nilai DECIMAL(5,2) NOT NULL DEFAULT 0.00;

-- Optional rollback (manual):
-- ALTER TABLE hasil_nilais MODIFY COLUMN total_benar INT NOT NULL DEFAULT 0;
-- ALTER TABLE hasil_nilais MODIFY COLUMN total_salah INT NOT NULL DEFAULT 0;
-- ALTER TABLE hasil_nilais MODIFY COLUMN total_pg INT NOT NULL DEFAULT 0;
-- ALTER TABLE hasil_nilais MODIFY COLUMN total_uraian INT NOT NULL DEFAULT 0;
-- ALTER TABLE hasil_nilais MODIFY COLUMN total_nilai INT NOT NULL DEFAULT 0;

