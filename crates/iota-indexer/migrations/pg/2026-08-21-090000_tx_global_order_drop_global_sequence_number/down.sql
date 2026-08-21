ALTER TABLE tx_global_order ADD COLUMN global_sequence_number BIGINT;
UPDATE tx_global_order SET global_sequence_number = COALESCE(tx_sequence_number, 0);
UPDATE tx_global_order g
SET global_sequence_number = o.global_sequence_number
FROM optimistic_transactions o
WHERE o.optimistic_sequence_number = g.optimistic_sequence_number;
ALTER TABLE tx_global_order ALTER COLUMN global_sequence_number SET NOT NULL;
CREATE UNIQUE INDEX tx_global_order_seq_digest ON tx_global_order (global_sequence_number, optimistic_sequence_number);
