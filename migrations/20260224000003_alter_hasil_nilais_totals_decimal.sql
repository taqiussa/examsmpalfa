-- Migration: change hasil_nilais.total_benar and total_salah to DECIMAL(5,2)

-- Normalize existing values to numeric and ensure non-null
UPDATE hasil_nilais
SET total_benar = COALESCE(total_benar, 0),
    total_salah = COALESCE(total_salah, 0),
    total_pg = COALESCE(total_pg, 0);

ALTER TABLE hasil_nilais
    MODIFY COLUMN total_benar DECIMAL(5,2) NOT NULL DEFAULT 0.00,
    MODIFY COLUMN total_salah DECIMAL(5,2) NOT NULL DEFAULT 0.00,
    MODIFY COLUMN total_pg DECIMAL(5,2) NOT NULL DEFAULT 0.00;

-- After running this migration restart the application and verify behavior.
