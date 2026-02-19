-- Migration: Alter soals.pertanyaan to LONGTEXT
-- Up: make column LONGTEXT to allow large embedded HTML/base64 images
ALTER TABLE soals MODIFY pertanyaan LONGTEXT NOT NULL;

-- Down: revert to TEXT (use with caution — may truncate data)
-- ALTER TABLE soals MODIFY pertanyaan TEXT NOT NULL;
