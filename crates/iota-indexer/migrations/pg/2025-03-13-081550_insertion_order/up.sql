-- Insertion order number that each transaction (either optimistic or
-- checkpointed) is assigned when being indexed. It provides common
-- ordering for optimistic and checkpointed transactions, whereas
-- `tx_sequence_number` provides ordering only for checkpointed transactions.
--
-- In case of missing digests, the `tx_digests` table is used a fallback
-- to resolve the transaction order. This is ok because optimistic transactions
-- will be inserted only after creation of this table.
CREATE SEQUENCE tx_insertion_order_seq;
CREATE TABLE tx_insertion_order (
    tx_digest                   BYTEA        PRIMARY KEY,
    insertion_order             BIGINT       NOT NULL DEFAULT nextval('tx_insertion_order_seq')
);
ALTER SEQUENCE tx_insertion_order_seq OWNED BY tx_insertion_order.insertion_order;
SELECT setval('tx_insertion_order_seq', (SELECT MAX(tx_sequence_number) FROM tx_digests));
CREATE UNIQUE INDEX tx_insertion_order_insertion_order ON tx_insertion_order (insertion_order);
