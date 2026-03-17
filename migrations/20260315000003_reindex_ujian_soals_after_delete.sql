-- Migration: reindex ujian_soals.urutan after delete so numbering stays contiguous

DROP TRIGGER IF EXISTS after_ujian_soal_delete;

CREATE TRIGGER after_ujian_soal_delete
AFTER DELETE ON ujian_soals
FOR EACH ROW
BEGIN
    SET @rownum := 0;

    UPDATE ujian_soals target
    JOIN (
        SELECT id, (@rownum := @rownum + 1) AS new_urutan
        FROM ujian_soals
        WHERE ujian_id = OLD.ujian_id
        ORDER BY urutan ASC, id ASC
    ) ordered ON ordered.id = target.id
    SET target.urutan = ordered.new_urutan
    WHERE target.ujian_id = OLD.ujian_id;

    UPDATE ujians
    SET total_soal = (
        SELECT COUNT(*)
        FROM ujian_soals
        WHERE ujian_id = OLD.ujian_id
    ),
    updated_at = NOW()
    WHERE id = OLD.ujian_id;
END;
