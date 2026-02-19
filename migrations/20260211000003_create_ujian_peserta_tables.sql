-- Migration: add token-based exam session tables for siswa flow

CREATE TABLE IF NOT EXISTS ujian_tokens (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    ujian_id BIGINT NOT NULL,
    token VARCHAR(32) NOT NULL,
    is_active BOOLEAN DEFAULT TRUE,
    expired_at DATETIME NULL,
    created_by BIGINT NULL,
    created_at DATETIME NOT NULL DEFAULT NOW(),
    UNIQUE KEY unique_exam_token (token),
    KEY idx_ujian_tokens_exam_active (ujian_id, is_active),
    CONSTRAINT fk_ujian_tokens_exam
        FOREIGN KEY (ujian_id) REFERENCES ujians(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS ujian_pesertas (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    ujian_id BIGINT NOT NULL,
    nis VARCHAR(20) NOT NULL,
    token_id BIGINT NOT NULL,
    status ENUM('started', 'submitted') NOT NULL DEFAULT 'started',
    started_at DATETIME NOT NULL DEFAULT NOW(),
    submitted_at DATETIME NULL,
    last_nomor INT NOT NULL DEFAULT 1,
    total_nilai INT NULL,
    created_at DATETIME NOT NULL DEFAULT NOW(),
    updated_at DATETIME NOT NULL DEFAULT NOW(),
    UNIQUE KEY unique_exam_user (ujian_id, nis),
    KEY idx_ujian_pesertas_nis (nis),
    KEY idx_ujian_pesertas_status (ujian_id, status),
    CONSTRAINT fk_ujian_pesertas_exam
        FOREIGN KEY (ujian_id) REFERENCES ujians(id) ON DELETE CASCADE,
    CONSTRAINT fk_ujian_pesertas_token
        FOREIGN KEY (token_id) REFERENCES ujian_tokens(id) ON DELETE RESTRICT
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS ujian_jawabans (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    ujian_id BIGINT NOT NULL,
    nis VARCHAR(20) NOT NULL,
    soal_id BIGINT NOT NULL,
    pilihan ENUM('a', 'b', 'c', 'd') NOT NULL,
    is_benar BOOLEAN NOT NULL DEFAULT FALSE,
    bobot_nilai INT NOT NULL DEFAULT 0,
    created_at DATETIME NOT NULL DEFAULT NOW(),
    updated_at DATETIME NOT NULL DEFAULT NOW(),
    UNIQUE KEY unique_siswa_soal (ujian_id, nis, soal_id),
    KEY idx_ujian_jawabans_siswa (ujian_id, nis),
    CONSTRAINT fk_ujian_jawabans_ujian
        FOREIGN KEY (ujian_id) REFERENCES ujians(id) ON DELETE CASCADE,
    CONSTRAINT fk_ujian_jawabans_soal
        FOREIGN KEY (soal_id) REFERENCES soals(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
