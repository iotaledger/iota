-- First `optimistic_sequence_number` value belonging to this epoch: rows
-- with a lower value were indexed before this epoch started. Lets the
-- pruner translate an epoch retention boundary into an
-- `optimistic_transactions` delete range.
ALTER TABLE epochs ADD COLUMN first_optimistic_sequence_number BIGINT;

-- Backfill existing epochs, one COALESCE arm per source:
-- 1. the first optimistic transaction at or after the epoch's first transaction
-- 2. the next value the sequence will assign, for epochs newer than the last optimistic row
-- 3. 0, when no optimistic transaction was ever indexed
UPDATE epochs e
SET first_optimistic_sequence_number = COALESCE(
    (SELECT o.optimistic_sequence_number
     FROM optimistic_transactions o
     WHERE o.global_sequence_number >= e.first_tx_sequence_number
     ORDER BY o.global_sequence_number, o.optimistic_sequence_number
     LIMIT 1),
    pg_sequence_last_value(
        pg_get_serial_sequence('tx_global_order', 'optimistic_sequence_number')) + 1,
    0
);

-- Fills `first_optimistic_sequence_number` on insert with the next
-- `tx_global_order` sequence value, or 0 if the sequence was never used.
ALTER TABLE epochs
ALTER COLUMN first_optimistic_sequence_number
SET DEFAULT COALESCE(pg_sequence_last_value(
    pg_get_serial_sequence('tx_global_order', 'optimistic_sequence_number')) + 1, 0);

ALTER TABLE epochs ALTER COLUMN first_optimistic_sequence_number SET NOT NULL;

-- The old bounds are in the `global_sequence_number` (tx) domain; reset them
-- so pruning restarts on the `optimistic_sequence_number` key.
WITH bounds AS (
    SELECT COALESCE(MIN(optimistic_sequence_number), 0) AS min_seq
    FROM optimistic_transactions
)
UPDATE watermarks
SET lowest_unpruned_key = bounds.min_seq,
    min_available_tx = bounds.min_seq
FROM bounds
WHERE entity = 'optimistic_transactions';
