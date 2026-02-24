-- Migration: change hasil_nilais.total_nilai to DECIMAL(5,2)
-- This migration only modifies the `total_nilai` column, leaving other columns untouched.

-- Normalize existing values to numeric and ensure non-null
UPDATE hasil_nilais
SET total_nilai = COALESCE(total_nilai, 0);

ALTER TABLE hasil_nilais
    MODIFY COLUMN total_nilai DECIMAL(5,2) NOT NULL DEFAULT 0.00;

-- Optional rollback (manual):
-- ALTER TABLE hasil_nilais MODIFY COLUMN total_nilai INT NOT NULL DEFAULT 0;
