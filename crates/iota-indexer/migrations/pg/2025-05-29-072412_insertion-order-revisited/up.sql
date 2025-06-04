DROP TABLE IF EXISTS tx_insertion_order;

-- It provides common ordering for optimistic and checkpointed transactions, whereas
-- `tx_digests.tx_sequence_number` provides ordering only for checkpointed transactions.
--
-- The `sequence_number` in this table defaults to the transaction sequence
-- number assigned in the checkpoint for checkpointed transactions, and to the
-- `SELECT MAX(tx_sequence_number) FROM tx_digests` at the time of insertion
-- for optimistic transactions.
--
-- Deterministic global order is guaranteed by the composite index on
-- `(global_sequence_number, optimistic_sequence_number)`, where
-- `optimistic_sequence_number` is the monotically increasing number
-- that represents the order of execution for optimistic transactions.
--
-- In case of missing digests, the `tx_digests` table is used as a fallback
-- to resolve the transaction order. This is ok because optimistic transactions
-- will be inserted only after creation of this table.
CREATE TABLE tx_global_order (
    tx_digest               BYTEA        PRIMARY KEY,
    global_sequence_number  BIGINT       NOT NULL,
    optimistic_sequence_number     BIGSERIAL
);
CREATE UNIQUE INDEX tx_global_order_seq_digest ON tx_global_order (global_sequence_number, optimistic_sequence_number);

ALTER TABLE optimistic_transactions                 RENAME insertion_order TO sequence_number;
ALTER TABLE optimistic_tx_senders                   RENAME tx_insertion_order TO sequence_number;
ALTER TABLE optimistic_tx_recipients                RENAME tx_insertion_order TO sequence_number;
ALTER TABLE optimistic_tx_input_objects             RENAME tx_insertion_order TO sequence_number;
ALTER TABLE optimistic_tx_changed_objects           RENAME tx_insertion_order TO sequence_number;
ALTER TABLE optimistic_tx_calls_pkg                 RENAME tx_insertion_order TO sequence_number;
ALTER TABLE optimistic_tx_calls_mod                 RENAME tx_insertion_order TO sequence_number;
ALTER TABLE optimistic_tx_calls_fun                 RENAME tx_insertion_order TO sequence_number;
ALTER TABLE optimistic_tx_kinds                     RENAME tx_insertion_order TO sequence_number;

ALTER TABLE optimistic_events                       RENAME tx_insertion_order TO sequence_number;
ALTER TABLE optimistic_event_emit_package           RENAME tx_insertion_order TO sequence_number;
ALTER TABLE optimistic_event_emit_module            RENAME tx_insertion_order TO sequence_number;
ALTER TABLE optimistic_event_struct_package         RENAME tx_insertion_order TO sequence_number;
ALTER TABLE optimistic_event_struct_module          RENAME tx_insertion_order TO sequence_number;
ALTER TABLE optimistic_event_struct_name            RENAME tx_insertion_order TO sequence_number;
ALTER TABLE optimistic_event_struct_instantiation   RENAME tx_insertion_order TO sequence_number;
ALTER TABLE optimistic_event_senders                RENAME tx_insertion_order TO sequence_number;
