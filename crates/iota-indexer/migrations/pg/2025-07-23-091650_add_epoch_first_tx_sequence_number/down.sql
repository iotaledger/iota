ALTER TABLE epochs RENAME COLUMN network_total_transactions TO epoch_total_transactions;
UPDATE epochs
SET epoch_total_transactions = epoch_total_transactions - first_tx_sequence_number;

ALTER TABLE epochs DROP COLUMN first_tx_sequence_number;
