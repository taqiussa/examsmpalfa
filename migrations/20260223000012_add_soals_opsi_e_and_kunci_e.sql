-- Migration: Add opsi_e and extend kunci/pilihan enums to include 'e'
-- Adds new option column and updates enums so existing data remains valid

ALTER TABLE soals
    ADD COLUMN opsi_e LONGTEXT AFTER opsi_d,
    MODIFY kunci_jawaban ENUM('a','b','c','d','e') NULL;

ALTER TABLE ujian_jawabans
    MODIFY pilihan ENUM('a','b','c','d','e') NULL;

-- Down (manual):
-- ALTER TABLE ujian_jawabans MODIFY pilihan ENUM('a','b','c','d') NULL;
-- ALTER TABLE soals MODIFY kunci_jawaban ENUM('a','b','c','d') NULL;
-- ALTER TABLE soals DROP COLUMN opsi_e;
