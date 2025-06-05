CREATE TABLE transaction_indexing_status (
    tx_digest                   BYTEA        PRIMARY KEY,
    indexing_completed          BOOLEAN      NOT NULL DEFAULT FALSE
);
