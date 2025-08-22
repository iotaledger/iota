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

-- Lookup table to search for optimistic transactions by sender address.
-- Equivalent of `tx_senders` table.
CREATE TABLE optimistic_tx_senders (
    global_sequence_number BIGINT NOT NULL,
    optimistic_sequence_number          BIGINT       NOT NULL,
    sender                      BYTEA        NOT NULL,
    PRIMARY KEY(sender, global_sequence_number, optimistic_sequence_number),
    FOREIGN KEY (global_sequence_number, optimistic_sequence_number)
    REFERENCES optimistic_transactions(global_sequence_number, optimistic_sequence_number)
    ON DELETE CASCADE
);

-- Lookup table to search for optimistic transactions by recipient address.
-- Equivalent of `tx_recipients` table.
CREATE TABLE optimistic_tx_recipients (
    global_sequence_number BIGINT NOT NULL,
    optimistic_sequence_number          BIGINT       NOT NULL,
    recipient                   BYTEA        NOT NULL,
    sender                      BYTEA        NOT NULL,
    PRIMARY KEY(recipient, global_sequence_number, optimistic_sequence_number),
    FOREIGN KEY (global_sequence_number, optimistic_sequence_number)
    REFERENCES optimistic_transactions(global_sequence_number, optimistic_sequence_number)
    ON DELETE CASCADE
);
CREATE INDEX optimistic_tx_recipients_sender ON optimistic_tx_recipients (sender, recipient, global_sequence_number, optimistic_sequence_number);

-- Lookup table to search for optimistic transactions by transaction input.
-- Equivalent of `tx_input_objects` table.
CREATE TABLE optimistic_tx_input_objects (
    global_sequence_number BIGINT NOT NULL,
    optimistic_sequence_number          BIGINT       NOT NULL,
    object_id                   BYTEA        NOT NULL,
    sender                      BYTEA        NOT NULL,
    PRIMARY KEY(object_id, global_sequence_number, optimistic_sequence_number),
    FOREIGN KEY (global_sequence_number, optimistic_sequence_number)
    REFERENCES optimistic_transactions(global_sequence_number, optimistic_sequence_number)
    ON DELETE CASCADE
);
CREATE INDEX optimistic_tx_input_objects_optimistic_sequence_number_index ON optimistic_tx_input_objects (global_sequence_number, optimistic_sequence_number);
CREATE INDEX optimistic_tx_input_objects_sender ON optimistic_tx_input_objects (sender, object_id, global_sequence_number, optimistic_sequence_number);

-- Lookup table to search for optimistic transactions by objects modified by transaction.
-- Equivalent of `tx_changed_objects` table.
CREATE TABLE optimistic_tx_changed_objects (
    global_sequence_number BIGINT NOT NULL,
    optimistic_sequence_number          BIGINT       NOT NULL,
    object_id                   BYTEA        NOT NULL,
    sender                      BYTEA        NOT NULL,
    PRIMARY KEY(object_id, global_sequence_number, optimistic_sequence_number),
    FOREIGN KEY (global_sequence_number, optimistic_sequence_number)
    REFERENCES optimistic_transactions(global_sequence_number, optimistic_sequence_number)
    ON DELETE CASCADE
);
CREATE INDEX optimistic_tx_changed_objects_optimistic_sequence_number_index ON optimistic_tx_changed_objects (global_sequence_number, optimistic_sequence_number);
CREATE INDEX optimistic_tx_changed_objects_sender ON optimistic_tx_changed_objects (sender, object_id, global_sequence_number, optimistic_sequence_number);

-- Lookup table to search for optimistic transactions by packages (that contain functions called in given tx).
-- Equivalent of `tx_calls_pkg` table.
CREATE TABLE optimistic_tx_calls_pkg (
    global_sequence_number BIGINT NOT NULL,
    optimistic_sequence_number          BIGINT       NOT NULL,
    package                     BYTEA        NOT NULL,
    sender                      BYTEA        NOT NULL,
    PRIMARY KEY(package, global_sequence_number, optimistic_sequence_number),
    FOREIGN KEY (global_sequence_number, optimistic_sequence_number)
    REFERENCES optimistic_transactions(global_sequence_number, optimistic_sequence_number)
    ON DELETE CASCADE
);
CREATE INDEX optimistic_tx_calls_pkg_sender ON optimistic_tx_calls_pkg (sender, package, global_sequence_number, optimistic_sequence_number);

-- Lookup table to search for optimistic transactions by modules (that contain functions called in given tx).
-- Equivalent of `tx_calls_mod` table.
CREATE TABLE optimistic_tx_calls_mod (
    global_sequence_number BIGINT NOT NULL,
    optimistic_sequence_number          BIGINT       NOT NULL,
    package                     BYTEA        NOT NULL,
    module                      TEXT         NOT NULL,
    sender                      BYTEA        NOT NULL,
    PRIMARY KEY(package, module, global_sequence_number, optimistic_sequence_number),
    FOREIGN KEY (global_sequence_number, optimistic_sequence_number)
    REFERENCES optimistic_transactions(global_sequence_number, optimistic_sequence_number)
    ON DELETE CASCADE
);
CREATE INDEX optimistic_tx_calls_mod_sender ON optimistic_tx_calls_mod (sender, package, module, global_sequence_number, optimistic_sequence_number);

-- Lookup table to search for optimistic transactions by called functions.
-- Equivalent of `tx_calls_fun` table.
CREATE TABLE optimistic_tx_calls_fun (
    global_sequence_number BIGINT NOT NULL,
    optimistic_sequence_number          BIGINT       NOT NULL,
    package                     BYTEA        NOT NULL,
    module                      TEXT         NOT NULL,
    func                        TEXT         NOT NULL,
    sender                      BYTEA        NOT NULL,
    PRIMARY KEY(package, module, func, global_sequence_number, optimistic_sequence_number),
    FOREIGN KEY (global_sequence_number, optimistic_sequence_number)
    REFERENCES optimistic_transactions(global_sequence_number, optimistic_sequence_number)
    ON DELETE CASCADE
);
CREATE INDEX optimistic_tx_calls_fun_sender ON optimistic_tx_calls_fun (sender, package, module, func, global_sequence_number, optimistic_sequence_number);

-- Lookup table to search for optimistic transactions by transaction kind (ptb or system)
-- Equivalent of `tx_kinds` table.
CREATE TABLE optimistic_tx_kinds (
    global_sequence_number BIGINT NOT NULL,
    optimistic_sequence_number          BIGINT       NOT NULL,
    tx_kind                     SMALLINT     NOT NULL,
    PRIMARY KEY(tx_kind, global_sequence_number, optimistic_sequence_number),
    FOREIGN KEY (global_sequence_number, optimistic_sequence_number)
    REFERENCES optimistic_transactions(global_sequence_number, optimistic_sequence_number)
    ON DELETE CASCADE
);

CREATE TABLE optimistic_tx_wrapped_or_deleted_objects (
    global_sequence_number BIGINT NOT NULL,
    optimistic_sequence_number          BIGINT       NOT NULL,
    object_id                   BYTEA        NOT NULL,
    sender                      BYTEA        NOT NULL,
    PRIMARY KEY(object_id, global_sequence_number, optimistic_sequence_number),
    FOREIGN KEY (global_sequence_number, optimistic_sequence_number)
    REFERENCES optimistic_transactions(global_sequence_number, optimistic_sequence_number)
    ON DELETE CASCADE
);
CREATE INDEX optimistic_tx_wrapped_or_deleted_objects_sequence_number_index ON optimistic_tx_wrapped_or_deleted_objects (global_sequence_number, optimistic_sequence_number);
CREATE INDEX optimistic_tx_wrapped_or_deleted_objects_sender ON optimistic_tx_wrapped_or_deleted_objects (sender, object_id, global_sequence_number, optimistic_sequence_number);


-- EVENTS

-- Main table storing data about optimistically indexed events
-- (events produced by transactions that were executed by the indexer,
-- and indexed without waiting for the transaction to be checkpointed).
-- Equivalent of `events` table.
CREATE TABLE optimistic_events
(
    global_sequence_number BIGINT NOT NULL,
    optimistic_sequence_number          BIGINT       NOT NULL,
    event_sequence_number       BIGINT       NOT NULL,
    transaction_digest          bytea        NOT NULL,
    -- array of IotaAddress in bytes. All signers of the transaction.
    senders                     bytea[]      NOT NULL,
    -- bytes of the entry package ID. Notice that the package and module here
    -- are the package and module of the function that emitted the event, different
    -- from the package and module of the event type.
    package                     bytea        NOT NULL,
    -- entry module name
    module                      text         NOT NULL,
    -- StructTag in Display format, fully qualified including type parameters
    event_type                  text         NOT NULL,
    -- bcs of the Event contents (Event.contents)
    bcs                         BYTEA        NOT NULL,
    PRIMARY KEY(global_sequence_number, optimistic_sequence_number, event_sequence_number),
    FOREIGN KEY (global_sequence_number, optimistic_sequence_number)
    REFERENCES optimistic_transactions(global_sequence_number, optimistic_sequence_number)
    ON DELETE CASCADE
);
CREATE INDEX optimistic_events_package ON optimistic_events (package, global_sequence_number, optimistic_sequence_number, event_sequence_number);
CREATE INDEX optimistic_events_package_module ON optimistic_events (package, module, global_sequence_number, optimistic_sequence_number, event_sequence_number);
CREATE INDEX optimistic_events_event_type ON optimistic_events (event_type text_pattern_ops, global_sequence_number, optimistic_sequence_number, event_sequence_number);

-- Lookup table to search for optimistic events by emitting package address.
-- Equivalent of `event_emit_package` table.
CREATE TABLE optimistic_event_emit_package
(
    package                     BYTEA   NOT NULL,
    global_sequence_number BIGINT NOT NULL,
    optimistic_sequence_number          BIGINT  NOT NULL,
    event_sequence_number       BIGINT  NOT NULL,
    sender                      BYTEA   NOT NULL,
    PRIMARY KEY(package, global_sequence_number, optimistic_sequence_number, event_sequence_number),
    FOREIGN KEY (global_sequence_number, optimistic_sequence_number)
    REFERENCES optimistic_transactions(global_sequence_number, optimistic_sequence_number)
    ON DELETE CASCADE
);
CREATE INDEX optimistic_event_emit_package_sender ON optimistic_event_emit_package (sender, package, global_sequence_number, optimistic_sequence_number, event_sequence_number);

-- Lookup table to search for optimistic events by emitting module name.
-- Equivalent of `event_emit_module` table.
CREATE TABLE optimistic_event_emit_module
(
    package                     BYTEA   NOT NULL,
    module                      TEXT    NOT NULL,
    global_sequence_number BIGINT NOT NULL,
    optimistic_sequence_number          BIGINT  NOT NULL,
    event_sequence_number       BIGINT  NOT NULL,
    sender                      BYTEA   NOT NULL,
    PRIMARY KEY(package, module, global_sequence_number, optimistic_sequence_number, event_sequence_number),
    FOREIGN KEY (global_sequence_number, optimistic_sequence_number)
    REFERENCES optimistic_transactions(global_sequence_number, optimistic_sequence_number)
    ON DELETE CASCADE
);
CREATE INDEX optimistic_event_emit_module_sender ON optimistic_event_emit_module (sender, package, module, global_sequence_number, optimistic_sequence_number, event_sequence_number);

-- Lookup table to search for optimistic events by package address of emitted type.
-- Equivalent of `event_struct_package` table.
CREATE TABLE optimistic_event_struct_package
(
    package                     BYTEA   NOT NULL,
    global_sequence_number BIGINT NOT NULL,
    optimistic_sequence_number          BIGINT  NOT NULL,
    event_sequence_number       BIGINT  NOT NULL,
    sender                      BYTEA   NOT NULL,
    PRIMARY KEY(package, global_sequence_number, optimistic_sequence_number, event_sequence_number),
    FOREIGN KEY (global_sequence_number, optimistic_sequence_number)
    REFERENCES optimistic_transactions(global_sequence_number, optimistic_sequence_number)
    ON DELETE CASCADE
);
CREATE INDEX optimistic_event_struct_package_sender ON optimistic_event_struct_package (sender, package, global_sequence_number, optimistic_sequence_number, event_sequence_number);

-- Lookup table to search for optimistic events by module name of emitted type.
-- Equivalent of `event_struct_module` table.
CREATE TABLE optimistic_event_struct_module
(
    package                     BYTEA   NOT NULL,
    module                      TEXT    NOT NULL,
    global_sequence_number BIGINT NOT NULL,
    optimistic_sequence_number          BIGINT  NOT NULL,
    event_sequence_number       BIGINT  NOT NULL,
    sender                      BYTEA   NOT NULL,
    PRIMARY KEY(package, module, global_sequence_number, optimistic_sequence_number, event_sequence_number),
    FOREIGN KEY (global_sequence_number, optimistic_sequence_number)
    REFERENCES optimistic_transactions(global_sequence_number, optimistic_sequence_number)
    ON DELETE CASCADE
);
CREATE INDEX optimistic_event_struct_module_sender ON optimistic_event_struct_module (sender, package, module, global_sequence_number, optimistic_sequence_number, event_sequence_number);

-- Lookup table to search for optimistic events by emitted type name.
-- Equivalent of `event_struct_name` table.
CREATE TABLE optimistic_event_struct_name
(
    package                     BYTEA   NOT NULL,
    module                      TEXT    NOT NULL,
    type_name                   TEXT    NOT NULL,
    global_sequence_number BIGINT NOT NULL,
    optimistic_sequence_number          BIGINT  NOT NULL,
    event_sequence_number       BIGINT  NOT NULL,
    sender                      BYTEA   NOT NULL,
    PRIMARY KEY(package, module, type_name, global_sequence_number, optimistic_sequence_number, event_sequence_number),
    FOREIGN KEY (global_sequence_number, optimistic_sequence_number)
    REFERENCES optimistic_transactions(global_sequence_number, optimistic_sequence_number)
    ON DELETE CASCADE
);
CREATE INDEX optimistic_event_struct_name_sender ON optimistic_event_struct_name (sender, package, module, type_name, global_sequence_number, optimistic_sequence_number, event_sequence_number);

-- Lookup table to search for optimistic events by emitted type name with type parameters.
-- Equivalent of `event_struct_instantiation` table.
CREATE TABLE optimistic_event_struct_instantiation
(
    package                     BYTEA   NOT NULL,
    module                      TEXT    NOT NULL,
    type_instantiation          TEXT    NOT NULL,
    global_sequence_number BIGINT NOT NULL,
    optimistic_sequence_number          BIGINT  NOT NULL,
    event_sequence_number       BIGINT  NOT NULL,
    sender                      BYTEA   NOT NULL,
    PRIMARY KEY(package, module, type_instantiation, global_sequence_number, optimistic_sequence_number, event_sequence_number),
    FOREIGN KEY (global_sequence_number, optimistic_sequence_number)
    REFERENCES optimistic_transactions(global_sequence_number, optimistic_sequence_number)
    ON DELETE CASCADE
);
CREATE INDEX optimistic_event_struct_instantiation_sender ON optimistic_event_struct_instantiation (sender, package, module, type_instantiation, global_sequence_number, optimistic_sequence_number, event_sequence_number);

-- Lookup table to search for optimistic events by event sender address
-- Equivalent of `event_senders` table.
CREATE TABLE optimistic_event_senders
(
    sender                      BYTEA   NOT NULL,
    global_sequence_number BIGINT NOT NULL,
    optimistic_sequence_number          BIGINT  NOT NULL,
    event_sequence_number       BIGINT  NOT NULL,
    PRIMARY KEY(sender, global_sequence_number, optimistic_sequence_number, event_sequence_number),
    FOREIGN KEY (global_sequence_number, optimistic_sequence_number)
    REFERENCES optimistic_transactions(global_sequence_number, optimistic_sequence_number)
    ON DELETE CASCADE
);
