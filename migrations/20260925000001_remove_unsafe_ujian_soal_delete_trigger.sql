-- Question ordering and totals are maintained atomically by soal_delete in
-- the application.  A trigger may update `ujians`, but it must not update
-- `ujian_soals` while that same table is being deleted from: MySQL rejects
-- that with "Can't update table ... already used by the statement".
--
-- The previous migration attempted exactly that reindex and could therefore
-- make question deletion fail.  Drop it for existing installations.  Fresh
-- installations also run this after the old migration, leaving no unsafe
-- delete trigger behind.
DROP TRIGGER IF EXISTS after_ujian_soal_delete;
