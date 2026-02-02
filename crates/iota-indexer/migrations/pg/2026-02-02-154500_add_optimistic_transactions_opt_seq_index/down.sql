-- Drop the index on optimistic_sequence_number.
-- CONCURRENTLY prevents blocking writes during index removal.
-- This migration runs outside of a transaction (configured in metadata.toml).
DROP INDEX CONCURRENTLY IF EXISTS optimistic_transactions_opt_seq_idx;
