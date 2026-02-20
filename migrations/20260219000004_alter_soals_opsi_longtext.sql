-- Migration: Alter soals opsi fields to LONGTEXT (nullable kunci_jawaban)
-- Up: allow large embedded HTML/base64 images for options and nullable kunci_jawaban for Uraian
ALTER TABLE soals
    MODIFY opsi_a LONGTEXT,
    MODIFY opsi_b LONGTEXT,
    MODIFY opsi_c LONGTEXT,
    MODIFY opsi_d LONGTEXT,
    MODIFY kunci_jawaban ENUM('a', 'b', 'c', 'd') NULL;

-- Down: revert to TEXT (use with caution — may truncate data)
-- ALTER TABLE soals
--     MODIFY opsi_a TEXT,
--     MODIFY opsi_b TEXT,
--     MODIFY opsi_c TEXT,
--     MODIFY opsi_d TEXT;
-- ALTER TABLE soals
--     MODIFY kunci_jawaban ENUM('a', 'b', 'c', 'd') NOT NULL;
