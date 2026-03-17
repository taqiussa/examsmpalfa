-- Migration: remove jurusan from ujians for SMP workflow
-- Exams now only depend on tahun and mata_pelajaran_id.

SET @jurusan_exists := (
    SELECT COUNT(1)
    FROM information_schema.columns
    WHERE table_schema = DATABASE()
      AND table_name = 'ujians'
      AND column_name = 'jurusan'
);

SET @drop_jurusan_sql := IF(
    @jurusan_exists = 1,
    'ALTER TABLE ujians DROP COLUMN jurusan',
    'SELECT 1'
);
PREPARE stmt_drop_jurusan FROM @drop_jurusan_sql;
EXECUTE stmt_drop_jurusan;
DEALLOCATE PREPARE stmt_drop_jurusan;

SET @user_id_exists := (
    SELECT COUNT(1)
    FROM information_schema.columns
    WHERE table_schema = DATABASE()
      AND table_name = 'ujians'
      AND column_name = 'user_id'
);

SET @move_user_id_sql := IF(
    @user_id_exists = 1,
    'ALTER TABLE ujians MODIFY COLUMN user_id BIGINT UNSIGNED NULL DEFAULT NULL AFTER mata_pelajaran_id',
    'SELECT 1'
);
PREPARE stmt_move_user_id FROM @move_user_id_sql;
EXECUTE stmt_move_user_id;
DEALLOCATE PREPARE stmt_move_user_id;
