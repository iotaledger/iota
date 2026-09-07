-- Snapshot of the `tx_global_order` sequence taken when the epoch row
-- is persisted: rows with a lower `optimistic_sequence_number` were indexed
-- before this epoch started. Lets the pruner translate an epoch retention
-- boundary into an `optimistic_transactions` delete range.
--
-- NULL for epochs recorded before this column existed; pruning of
-- `optimistic_transactions` pauses until the retention boundary reaches an
-- epoch with a value.
ALTER TABLE epochs ADD COLUMN min_optimistic_sequence_number BIGINT DEFAULT NULL;
