-- Migration: backfill ujians.tahun from tanggal for legacy data
UPDATE ujians
SET tahun = CASE
    WHEN tanggal IS NULL THEN tahun
    WHEN MONTH(tanggal) >= 7 THEN CONCAT(YEAR(tanggal), ' / ', YEAR(tanggal) + 1)
    ELSE CONCAT(YEAR(tanggal) - 1, ' / ', YEAR(tanggal))
END
WHERE (tahun IS NULL OR tahun = '');
