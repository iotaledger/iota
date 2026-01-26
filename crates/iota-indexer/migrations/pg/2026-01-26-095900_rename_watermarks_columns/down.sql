-- Add reader_lo column back as nullable first
ALTER TABLE watermarks
ADD COLUMN reader_lo BIGINT;

-- Populate reader_lo based on the entity type
-- For checkpoint-based tables (objects_history, checkpoints, pruner_cp_watermark), use min_available_cp
-- For transaction-based tables (transactions, events, and all others), use min_available_tx
UPDATE watermarks
SET reader_lo = CASE
    WHEN entity IN ('objects_history', 'checkpoints', 'pruner_cp_watermark') THEN min_available_cp
    ELSE min_available_tx
END;

-- Make reader_lo NOT NULL after populating
ALTER TABLE watermarks
ALTER COLUMN reader_lo SET NOT NULL;

-- Rename min_bounds_updated_at_timestamp_ms back to timestamp_ms
ALTER TABLE watermarks
RENAME COLUMN min_bounds_updated_at_timestamp_ms TO timestamp_ms;

-- Rename lowest_unpruned_key back to pruner_hi
ALTER TABLE watermarks
RENAME COLUMN lowest_unpruned_key TO pruner_hi;

-- Rename min_available_epoch back to epoch_lo
ALTER TABLE watermarks
RENAME COLUMN min_available_epoch TO epoch_lo;

-- Update max_committed_tx back to exclusive by adding 1 (was inclusive)
UPDATE watermarks
SET max_committed_tx = max_committed_tx + 1;

-- Rename max_committed_tx back to tx_hi
ALTER TABLE watermarks
RENAME COLUMN max_committed_tx TO tx_hi;

-- Rename max_committed_cp back to checkpoint_hi_inclusive
ALTER TABLE watermarks
RENAME COLUMN max_committed_cp TO checkpoint_hi_inclusive;

-- Rename current_epoch back to epoch_hi_inclusive
ALTER TABLE watermarks
RENAME COLUMN current_epoch TO epoch_hi_inclusive;

-- Remove min_available_tx and min_available_cp columns from watermarks table
ALTER TABLE watermarks
DROP COLUMN IF EXISTS min_available_tx,
DROP COLUMN IF EXISTS min_available_cp;
