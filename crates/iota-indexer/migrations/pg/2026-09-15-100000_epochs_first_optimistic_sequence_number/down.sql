-- Restore `global_sequence_number` first: the watermark reset below reads it.
ALTER TABLE optimistic_transactions ADD COLUMN global_sequence_number BIGINT;
UPDATE optimistic_transactions
SET global_sequence_number = (SELECT COALESCE(MAX(tx_sequence_number), 0) FROM tx_global_order);
ALTER TABLE optimistic_transactions ALTER COLUMN global_sequence_number SET NOT NULL;
CREATE INDEX optimistic_transactions_global_seq ON optimistic_transactions (global_sequence_number);

ALTER TABLE epochs DROP COLUMN first_optimistic_sequence_number;
DROP FUNCTION next_optimistic_sequence_number();

-- The bounds were rewritten into the `optimistic_sequence_number` domain;
-- reset them so pruning restarts on the `global_sequence_number` key the
-- previous release uses.
WITH bounds AS (
    SELECT COALESCE(MIN(global_sequence_number), 0) AS min_seq
    FROM optimistic_transactions
)
UPDATE watermarks
SET lowest_unpruned_key = bounds.min_seq,
    min_available_tx = bounds.min_seq
FROM bounds
WHERE entity = 'optimistic_transactions';
