DROP TRIGGER IF EXISTS epochs_first_optimistic_seq ON epochs;
DROP FUNCTION epochs_set_first_optimistic_seq();
ALTER TABLE epochs DROP COLUMN first_optimistic_sequence_number;

-- The bounds were rewritten into the `optimistic_sequence_number` domain
-- reset them so pruning restarts on the `global_sequence_number` key the previous release uses.
WITH bounds AS (
    SELECT COALESCE(MIN(global_sequence_number), 0) AS min_seq
    FROM optimistic_transactions
)
UPDATE watermarks
SET lowest_unpruned_key = bounds.min_seq,
    min_available_tx = bounds.min_seq
FROM bounds
WHERE entity = 'optimistic_transactions';
