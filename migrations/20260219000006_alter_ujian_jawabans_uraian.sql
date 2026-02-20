-- Migration: allow uraian answers
ALTER TABLE ujian_jawabans
    MODIFY pilihan ENUM('a','b','c','d') NULL,
    ADD COLUMN jawaban_uraian LONGTEXT NULL AFTER pilihan;
