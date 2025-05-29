DROP TABLE IF EXISTS tx_global_order;

CREATE SEQUENCE tx_insertion_order_seq;
CREATE TABLE tx_insertion_order (
    tx_digest                   BYTEA        PRIMARY KEY,
    insertion_order             BIGINT       NOT NULL DEFAULT nextval('tx_insertion_order_seq')
);
ALTER SEQUENCE tx_insertion_order_seq OWNED BY tx_insertion_order.insertion_order;
SELECT setval('tx_insertion_order_seq', (SELECT MAX(tx_sequence_number) FROM tx_digests));
CREATE UNIQUE INDEX tx_insertion_order_insertion_order ON tx_insertion_order (insertion_order);

ALTER TABLE optimistic_transactions                 RENAME global_sequence_number TO insertion_order;
ALTER TABLE optimistic_tx_senders                   RENAME global_sequence_number TO tx_insertion_order;
ALTER TABLE optimistic_tx_recipients                RENAME global_sequence_number TO tx_insertion_order;
ALTER TABLE optimistic_tx_input_objects             RENAME global_sequence_number TO tx_insertion_order;
ALTER TABLE optimistic_tx_changed_objects           RENAME global_sequence_number TO tx_insertion_order;
ALTER TABLE optimistic_tx_calls_pkg                 RENAME global_sequence_number TO tx_insertion_order;
ALTER TABLE optimistic_tx_calls_mod                 RENAME global_sequence_number TO tx_insertion_order;
ALTER TABLE optimistic_tx_calls_fun                 RENAME global_sequence_number TO tx_insertion_order;
ALTER TABLE optimistic_tx_kinds                     RENAME global_sequence_number TO tx_insertion_order;

ALTER TABLE optimistic_events                       RENAME global_sequence_number TO tx_insertion_order;
ALTER TABLE optimistic_event_emit_package           RENAME global_sequence_number TO tx_insertion_order;
ALTER TABLE optimistic_event_emit_module            RENAME global_sequence_number TO tx_insertion_order;
ALTER TABLE optimistic_event_struct_package         RENAME global_sequence_number TO tx_insertion_order;
ALTER TABLE optimistic_event_struct_module          RENAME global_sequence_number TO tx_insertion_order;
ALTER TABLE optimistic_event_struct_name            RENAME global_sequence_number TO tx_insertion_order;
ALTER TABLE optimistic_event_struct_instantiation   RENAME global_sequence_number TO tx_insertion_order;
ALTER TABLE optimistic_event_senders                RENAME global_sequence_number TO tx_insertion_order;
