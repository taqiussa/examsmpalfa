-- Migration: remove opsi_e from soals now that questions use four choices (A-D)

SET @opsi_e_exists := (
    SELECT COUNT(1)
    FROM information_schema.columns
    WHERE table_schema = DATABASE()
      AND table_name = 'soals'
      AND column_name = 'opsi_e'
);

SET @drop_opsi_e_sql := IF(
    @opsi_e_exists = 1,
    'ALTER TABLE soals DROP COLUMN opsi_e',
    'SELECT 1'
);
PREPARE stmt_drop_opsi_e FROM @drop_opsi_e_sql;
EXECUTE stmt_drop_opsi_e;
DEALLOCATE PREPARE stmt_drop_opsi_e;
