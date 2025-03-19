INSERT INTO tx_insertion_order (tx_digest, insertion_order)
SELECT transaction_digest, tx_sequence_number FROM transactions;
