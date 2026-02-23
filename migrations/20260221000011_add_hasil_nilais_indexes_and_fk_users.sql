-- Migration: add indexes and optional FK for hasil_nilais.nis -> users.nis

-- Ensure helpful composite indexes on hasil_nilais
SET @idx1 := (
    SELECT COUNT(1)
    FROM information_schema.statistics
    WHERE table_schema = DATABASE()
      AND table_name = 'hasil_nilais'
      AND index_name = 'idx_hasil_nilais_mapel_tahun'
);
SET @sql1 := IF(@idx1 = 0,
    'CREATE INDEX idx_hasil_nilais_mapel_tahun ON hasil_nilais (mata_pelajaran_id, tahun)',
    'SELECT 1'
);
PREPARE stmt1 FROM @sql1; EXECUTE stmt1; DEALLOCATE PREPARE stmt1;

SET @idx2 := (
    SELECT COUNT(1)
    FROM information_schema.statistics
    WHERE table_schema = DATABASE()
      AND table_name = 'hasil_nilais'
      AND index_name = 'idx_hasil_nilais_nis_mapel_tahun'
);
SET @sql2 := IF(@idx2 = 0,
    'CREATE INDEX idx_hasil_nilais_nis_mapel_tahun ON hasil_nilais (nis, mata_pelajaran_id, tahun)',
    'SELECT 1'
);
PREPARE stmt2 FROM @sql2; EXECUTE stmt2; DEALLOCATE PREPARE stmt2;

-- Ensure users.nis is indexed (required for FK in MySQL)
SET @users_tbl := (
    SELECT COUNT(1)
    FROM information_schema.tables
    WHERE table_schema = DATABASE()
      AND table_name = 'users'
);

SET @users_nis_idx := (
    SELECT COUNT(1)
    FROM information_schema.statistics
    WHERE table_schema = DATABASE()
      AND table_name = 'users'
      AND index_name = 'idx_users_nis'
);

SET @sql3 := IF(@users_tbl = 1 AND @users_nis_idx = 0,
    'CREATE INDEX idx_users_nis ON users (nis)',
    'SELECT 1'
);
PREPARE stmt3 FROM @sql3; EXECUTE stmt3; DEALLOCATE PREPARE stmt3;

-- Add FK hasil_nilais.nis -> users.nis if possible
SET @fk_exists := (
    SELECT COUNT(1)
    FROM information_schema.table_constraints
    WHERE table_schema = DATABASE()
      AND table_name = 'hasil_nilais'
      AND constraint_name = 'fk_hasil_nilais_users'
      AND constraint_type = 'FOREIGN KEY'
);

SET @sql4 := IF(@users_tbl = 1 AND @fk_exists = 0,
    'ALTER TABLE hasil_nilais ADD CONSTRAINT fk_hasil_nilais_users FOREIGN KEY (nis) REFERENCES users(nis) ON DELETE RESTRICT',
    'SELECT 1'
);
PREPARE stmt4 FROM @sql4; EXECUTE stmt4; DEALLOCATE PREPARE stmt4;
