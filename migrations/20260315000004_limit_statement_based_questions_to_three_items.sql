-- Limit statement-based question models to 3 items without affecting regular pilihan ganda.
UPDATE soals
SET opsi_d = NULL,
    kunci_jawaban = CASE
        WHEN kunci_jawaban IS NULL OR TRIM(kunci_jawaban) = '' THEN kunci_jawaban
        ELSE SUBSTRING_INDEX(kunci_jawaban, ',', 3)
    END,
    updated_at = NOW()
WHERE kategori = 'Benar/Salah';

UPDATE soals
SET opsi_d = NULL,
    updated_at = NOW()
WHERE kategori = 'Pilihan Ganda Kompleks MCMA';
