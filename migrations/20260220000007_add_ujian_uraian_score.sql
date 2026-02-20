-- Migration: add uraian score and review status
ALTER TABLE ujian_jawabans
    ADD COLUMN nilai_uraian INT NOT NULL DEFAULT 0 AFTER jawaban_uraian,
    ADD COLUMN status_uraian ENUM('reviewed') NULL AFTER nilai_uraian;
