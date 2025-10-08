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
    optimistic_sequence_number     BIGSERIAL,
    chk_tx_sequence_number      BIGINT
);
CREATE UNIQUE INDEX tx_global_order_seq_digest ON tx_global_order (global_sequence_number, optimistic_sequence_number);
CREATE UNIQUE INDEX tx_global_order_chk_tx_seq_num ON tx_global_order (chk_tx_sequence_number);

DROP TABLE IF EXISTS optimistic_tx_senders;
DROP TABLE IF EXISTS optimistic_tx_recipients;
DROP TABLE IF EXISTS optimistic_tx_input_objects;
DROP TABLE IF EXISTS optimistic_tx_changed_objects;
DROP TABLE IF EXISTS optimistic_tx_calls_pkg;
DROP TABLE IF EXISTS optimistic_tx_calls_mod;
DROP TABLE IF EXISTS optimistic_tx_calls_fun;
DROP TABLE IF EXISTS optimistic_tx_kinds;

DROP TABLE IF EXISTS optimistic_event_emit_package;
DROP TABLE IF EXISTS optimistic_event_emit_module;
DROP TABLE IF EXISTS optimistic_event_struct_package;
DROP TABLE IF EXISTS optimistic_event_struct_module;
DROP TABLE IF EXISTS optimistic_event_struct_name;
DROP TABLE IF EXISTS optimistic_event_struct_instantiation;
DROP TABLE IF EXISTS optimistic_event_senders;
DROP TABLE IF EXISTS optimistic_events;

DROP TABLE IF EXISTS optimistic_transactions;



-- TRANSACTIONS

-- Main table storing data about optimistically indexed transactions
-- (transactions that were executed by the indexer, and indexed without waiting for them to be checkpointed).
-- Equivalent of `transactions` table.
CREATE TABLE optimistic_transactions (
    global_sequence_number BIGINT NOT NULL,
    optimistic_sequence_number             BIGINT NOT NULL,
    transaction_digest          bytea        NOT NULL,
    -- bcs serialized SenderSignedData bytes
    raw_transaction             bytea        NOT NULL,
    -- bcs serialized TransactionEffects bytes
    raw_effects                 bytea        NOT NULL,
    -- array of bcs serialized IndexedObjectChange bytes
    object_changes              bytea[]      NOT NULL,
    -- array of bcs serialized BalanceChange bytes
    balance_changes             bytea[]      NOT NULL,
    -- array of bcs serialized StoredEvent bytes
    events                      bytea[]      NOT NULL,
    -- SystemTransaction/ProgrammableTransaction. See types.rs
    transaction_kind            smallint     NOT NULL,
    -- number of successful commands in this transaction, bound by number of command
    -- in a programmable transaction.
    success_command_count       smallint     NOT NULL,
    PRIMARY KEY(global_sequence_number, optimistic_sequence_number)
);
