-- First `optimistic_sequence_number` value belonging to this epoch: rows
-- with a lower value were indexed before this epoch started. Lets the
-- pruner translate an epoch retention boundary into an
-- `optimistic_transactions` delete range.
ALTER TABLE epochs ADD COLUMN first_optimistic_sequence_number BIGINT;

-- Next value the `tx_global_order.optimistic_sequence_number` sequence will
-- assign, or 0 if it was never used.
CREATE FUNCTION next_optimistic_sequence_number() RETURNS BIGINT
LANGUAGE sql AS $$
    SELECT COALESCE(
        (SELECT s.last_value + 1
         FROM pg_sequences s
         -- Resolve the sequence dynamically instead of hardcoding its
         -- generated name, so the lookup works even if sequence renames.
         WHERE format('%I.%I', s.schemaname, s.sequencename)::regclass
             = pg_get_serial_sequence('tx_global_order', 'optimistic_sequence_number')::regclass),
        0)b
$$;

-- Backfill existing epochs, one COALESCE arm per source:
-- 1. the first optimistic transaction at or after the epoch's first transaction
-- 2. the next value the sequence will assign, for epochs newer than the last
--    optimistic row, or 0 when no optimistic transaction was ever indexed
UPDATE epochs e
SET first_optimistic_sequence_number = COALESCE(
    (SELECT o.optimistic_sequence_number
     FROM optimistic_transactions o
     WHERE o.global_sequence_number >= e.first_tx_sequence_number
     ORDER BY o.global_sequence_number, o.optimistic_sequence_number
     LIMIT 1),
    next_optimistic_sequence_number()
);

-- Fills `first_optimistic_sequence_number` on insert when the writer does
-- not supply a value.
ALTER TABLE epochs
ALTER COLUMN first_optimistic_sequence_number
SET DEFAULT next_optimistic_sequence_number();

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
