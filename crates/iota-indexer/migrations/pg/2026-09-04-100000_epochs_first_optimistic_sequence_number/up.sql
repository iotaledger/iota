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

CREATE FUNCTION epochs_set_first_optimistic_seq() RETURNS trigger AS $$
BEGIN
    IF NEW.first_optimistic_sequence_number IS NULL THEN
        NEW.first_optimistic_sequence_number := pg_sequence_last_value(
            pg_get_serial_sequence('tx_global_order', 'optimistic_sequence_number')) + 1;
    END IF;
    RETURN NEW;
END $$ LANGUAGE plpgsql;

CREATE TRIGGER epochs_first_optimistic_seq
BEFORE INSERT ON epochs
FOR EACH ROW EXECUTE FUNCTION epochs_set_first_optimistic_seq();

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
