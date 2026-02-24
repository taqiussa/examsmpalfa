-- Add user_id column to ujians to track creator
ALTER TABLE ujians
  ADD COLUMN user_id BIGINT UNSIGNED NULL DEFAULT NULL AFTER jurusan;

-- add an index to speed up lookups by user
CREATE INDEX IF NOT EXISTS idx_ujians_user_id ON ujians(user_id);

-- Note: we intentionally do not add a foreign key constraint to avoid migration failures
-- if the users table or column types differ across deployments. Backfill should be
-- performed separately if needed.
