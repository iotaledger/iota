-- First `optimistic_sequence_number` value belonging to this epoch: rows
-- with a lower value were indexed before this epoch started. Lets the
-- pruner translate an epoch retention boundary into an
-- `optimistic_transactions` delete range.
--
-- NULL for epochs recorded before this column existed, or when no
-- optimistic transaction had been indexed yet; pruning of
-- `optimistic_transactions` pauses until the retention boundary reaches an
-- epoch with a value.
ALTER TABLE epochs ADD COLUMN first_optimistic_sequence_number BIGINT DEFAULT NULL;

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
