CREATE TABLE tx_digests (
    tx_digest                   BYTEA        PRIMARY KEY,
    tx_sequence_number          BIGINT       NOT NULL
);
CREATE INDEX tx_digests_tx_sequence_number ON tx_digests (tx_sequence_number);
ALTER TABLE tx_digests SET (autovacuum_vacuum_scale_factor=0.01, autovacuum_vacuum_cost_limit=500);
