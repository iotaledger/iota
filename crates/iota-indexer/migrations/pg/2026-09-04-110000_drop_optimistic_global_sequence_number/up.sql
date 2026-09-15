-- pruning now deletes by `optimistic_sequence_number` bounded by
-- `epochs.min_optimistic_sequence_number` making `global_sequence_number` obsolete.
DROP INDEX optimistic_transactions_global_seq;
ALTER TABLE optimistic_transactions DROP COLUMN global_sequence_number;
