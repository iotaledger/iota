-- Add index on optimistic_sequence_number to support efficient pruning.
-- This allows the DELETE operation to quickly find rows within a range
-- of optimistic_sequence_number values without requiring a full table scan.
--
-- CONCURRENTLY prevents blocking writes during index creation.
-- This migration runs outside of a transaction (configured in metadata.toml).
CREATE INDEX CONCURRENTLY optimistic_transactions_opt_seq_idx
ON optimistic_transactions (optimistic_sequence_number);
