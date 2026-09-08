ALTER TABLE optimistic_transactions ADD COLUMN global_sequence_number BIGINT;
UPDATE optimistic_transactions
SET global_sequence_number = (SELECT COALESCE(MAX(tx_sequence_number), 0) FROM tx_global_order);
ALTER TABLE optimistic_transactions ALTER COLUMN global_sequence_number SET NOT NULL;
CREATE INDEX optimistic_transactions_global_seq ON optimistic_transactions (global_sequence_number);
