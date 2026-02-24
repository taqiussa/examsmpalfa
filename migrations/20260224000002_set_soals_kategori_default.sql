-- Migration: Set default kategori for soals and backfill existing NULLs
ALTER TABLE soals
    MODIFY kategori VARCHAR(100) NOT NULL DEFAULT 'Pilihan Ganda';

-- Backfill existing NULL kategori rows to 'Pilihan Ganda'
UPDATE soals SET kategori = 'Pilihan Ganda' WHERE kategori IS NULL OR kategori = ''; 
