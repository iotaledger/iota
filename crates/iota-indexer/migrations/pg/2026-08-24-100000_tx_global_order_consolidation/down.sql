DROP INDEX optimistic_transactions_global_seq;
ALTER TABLE optimistic_transactions DROP CONSTRAINT optimistic_transactions_pkey;
ALTER TABLE optimistic_transactions ADD PRIMARY KEY (global_sequence_number, optimistic_sequence_number);

ALTER TABLE tx_global_order ADD COLUMN global_sequence_number BIGINT;
UPDATE tx_global_order SET global_sequence_number = COALESCE(tx_sequence_number, 0);
UPDATE tx_global_order g
SET global_sequence_number = o.global_sequence_number
FROM optimistic_transactions o
WHERE o.optimistic_sequence_number = g.optimistic_sequence_number;
ALTER TABLE tx_global_order ALTER COLUMN global_sequence_number SET NOT NULL;
CREATE UNIQUE INDEX tx_global_order_seq_digest ON tx_global_order (global_sequence_number, optimistic_sequence_number);

ALTER INDEX tx_global_order_tx_seq_num RENAME TO tx_global_order_chk_tx_seq_num;
ALTER TABLE tx_global_order RENAME COLUMN tx_sequence_number TO chk_tx_sequence_number;

CREATE TABLE tx_digests (
    tx_digest                   BYTEA        PRIMARY KEY,
    tx_sequence_number          BIGINT       NOT NULL
);
CREATE INDEX tx_digests_tx_sequence_number ON tx_digests (tx_sequence_number);
ALTER TABLE tx_digests SET (autovacuum_vacuum_scale_factor=0.01, autovacuum_vacuum_cost_limit=500);
