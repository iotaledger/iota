-- Add min_available_tx and min_available_cp columns to watermarks table
ALTER TABLE watermarks
ADD COLUMN min_available_tx BIGINT,
ADD COLUMN min_available_cp BIGINT;

-- Populate initial values for min_available_tx and min_available_cp based on epoch_lo
-- For each watermark entry, find the corresponding epoch and use its first_checkpoint_id
-- and first_tx_sequence_number
UPDATE watermarks w
SET
    min_available_cp = COALESCE(e.first_checkpoint_id, 0),
    min_available_tx = COALESCE(e.first_tx_sequence_number, 0)
FROM epochs e
WHERE e.epoch = w.epoch_lo;

-- For watermarks where epoch_lo doesn't exist in epochs table (shouldn't happen in normal operation),
-- set to 0 as a safe default
UPDATE watermarks
SET
    min_available_cp = COALESCE(min_available_cp, 0),
    min_available_tx = COALESCE(min_available_tx, 0)
WHERE min_available_cp IS NULL OR min_available_tx IS NULL;

-- Make columns NOT NULL after populating them
ALTER TABLE watermarks
ALTER COLUMN min_available_tx SET NOT NULL,
ALTER COLUMN min_available_cp SET NOT NULL;

-- Drop reader_lo column as it's replaced by min_available_tx and min_available_cp
ALTER TABLE watermarks
DROP COLUMN reader_lo;

-- Rename epoch_hi_inclusive to current_epoch for clarity
ALTER TABLE watermarks
RENAME COLUMN epoch_hi_inclusive TO current_epoch;

-- Rename checkpoint_hi_inclusive to max_committed_cp for clarity
ALTER TABLE watermarks
RENAME COLUMN checkpoint_hi_inclusive TO max_committed_cp;

-- Rename tx_hi to max_committed_tx and change from exclusive to inclusive
ALTER TABLE watermarks
RENAME COLUMN tx_hi TO max_committed_tx;

-- Update max_committed_tx to be inclusive by subtracting 1 (was exclusive upper bound)
UPDATE watermarks
SET max_committed_tx = max_committed_tx - 1
WHERE max_committed_tx > 0;

-- Rename epoch_lo to min_available_epoch for clarity
ALTER TABLE watermarks
RENAME COLUMN epoch_lo TO min_available_epoch;

-- Rename pruner_hi to lowest_unpruned_key for clarity
ALTER TABLE watermarks
RENAME COLUMN pruner_hi TO lowest_unpruned_key;

-- Rename timestamp_ms to min_bounds_updated_at_timestamp_ms for clarity
ALTER TABLE watermarks
RENAME COLUMN timestamp_ms TO min_bounds_updated_at_timestamp_ms;
