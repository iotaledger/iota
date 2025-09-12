DROP TABLE IF EXISTS tx_global_order;

CREATE SEQUENCE tx_insertion_order_seq;
CREATE TABLE tx_insertion_order (
    tx_digest                   BYTEA        PRIMARY KEY,
    insertion_order             BIGINT       NOT NULL DEFAULT nextval('tx_insertion_order_seq')
);
ALTER SEQUENCE tx_insertion_order_seq OWNED BY tx_insertion_order.insertion_order;
SELECT setval('tx_insertion_order_seq', (SELECT MAX(tx_sequence_number) FROM tx_digests));
CREATE UNIQUE INDEX tx_insertion_order_insertion_order ON tx_insertion_order (insertion_order);

DROP TABLE IF EXISTS optimistic_transactions;

-- Main table storing data about optimistically indexed transactions
-- (transactions that were executed by the indexer, and indexed without waiting for them to be checkpointed).
-- Equivalent of `transactions` table.
CREATE TABLE optimistic_transactions (
    insertion_order             BIGINT       PRIMARY KEY,
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
    success_command_count       smallint     NOT NULL
);

-- Lookup table to search for optimistic transactions by sender address.
-- Equivalent of `tx_senders` table.
CREATE TABLE optimistic_tx_senders (
    tx_insertion_order          BIGINT       REFERENCES optimistic_transactions(insertion_order) ON DELETE CASCADE,
    sender                      BYTEA        NOT NULL,
    PRIMARY KEY(sender, tx_insertion_order)
);

-- Lookup table to search for optimistic transactions by recipient address.
-- Equivalent of `tx_recipients` table.
CREATE TABLE optimistic_tx_recipients (
    tx_insertion_order          BIGINT       REFERENCES optimistic_transactions(insertion_order) ON DELETE CASCADE,
    recipient                   BYTEA        NOT NULL,
    sender                      BYTEA        NOT NULL,
    PRIMARY KEY(recipient, tx_insertion_order)
);
CREATE INDEX optimistic_tx_recipients_sender ON optimistic_tx_recipients (sender, recipient, tx_insertion_order);

-- Lookup table to search for optimistic transactions by transaction input.
-- Equivalent of `tx_input_objects` table.
CREATE TABLE optimistic_tx_input_objects (
    tx_insertion_order          BIGINT       REFERENCES optimistic_transactions(insertion_order) ON DELETE CASCADE,
    object_id                   BYTEA        NOT NULL,
    sender                      BYTEA        NOT NULL,
    PRIMARY KEY(object_id, tx_insertion_order)
);
CREATE INDEX optimistic_tx_input_objects_tx_insertion_order_index ON optimistic_tx_input_objects (tx_insertion_order);
CREATE INDEX optimistic_tx_input_objects_sender ON optimistic_tx_input_objects (sender, object_id, tx_insertion_order);

-- Lookup table to search for optimistic transactions by objects modified by transaction.
-- Equivalent of `tx_changed_objects` table.
CREATE TABLE optimistic_tx_changed_objects (
    tx_insertion_order          BIGINT       REFERENCES optimistic_transactions(insertion_order) ON DELETE CASCADE,
    object_id                   BYTEA        NOT NULL,
    sender                      BYTEA        NOT NULL,
    PRIMARY KEY(object_id, tx_insertion_order)
);
CREATE INDEX optimistic_tx_changed_objects_tx_insertion_order_index ON optimistic_tx_changed_objects (tx_insertion_order);
CREATE INDEX optimistic_tx_changed_objects_sender ON optimistic_tx_changed_objects (sender, object_id, tx_insertion_order);

-- Lookup table to search for optimistic transactions by packages (that contain functions called in given tx).
-- Equivalent of `tx_calls_pkg` table.
CREATE TABLE optimistic_tx_calls_pkg (
    tx_insertion_order          BIGINT       REFERENCES optimistic_transactions(insertion_order) ON DELETE CASCADE,
    package                     BYTEA        NOT NULL,
    sender                      BYTEA        NOT NULL,
    PRIMARY KEY(package, tx_insertion_order)
);
CREATE INDEX optimistic_tx_calls_pkg_sender ON optimistic_tx_calls_pkg (sender, package, tx_insertion_order);

-- Lookup table to search for optimistic transactions by modules (that contain functions called in given tx).
-- Equivalent of `tx_calls_mod` table.
CREATE TABLE optimistic_tx_calls_mod (
    tx_insertion_order          BIGINT       REFERENCES optimistic_transactions(insertion_order) ON DELETE CASCADE,
    package                     BYTEA        NOT NULL,
    module                      TEXT         NOT NULL,
    sender                      BYTEA        NOT NULL,
    PRIMARY KEY(package, module, tx_insertion_order)
);
CREATE INDEX optimistic_tx_calls_mod_sender ON optimistic_tx_calls_mod (sender, package, module, tx_insertion_order);

-- Lookup table to search for optimistic transactions by called functions.
-- Equivalent of `tx_calls_fun` table.
CREATE TABLE optimistic_tx_calls_fun (
    tx_insertion_order          BIGINT       REFERENCES optimistic_transactions(insertion_order) ON DELETE CASCADE,
    package                     BYTEA        NOT NULL,
    module                      TEXT         NOT NULL,
    func                        TEXT         NOT NULL,
    sender                      BYTEA        NOT NULL,
    PRIMARY KEY(package, module, func, tx_insertion_order)
);
CREATE INDEX optimistic_tx_calls_fun_sender ON optimistic_tx_calls_fun (sender, package, module, func, tx_insertion_order);

-- Lookup table to search for optimistic transactions by transaction kind (ptb or system)
-- Equivalent of `tx_kinds` table.
CREATE TABLE optimistic_tx_kinds (
    tx_insertion_order          BIGINT       REFERENCES optimistic_transactions(insertion_order) ON DELETE CASCADE,
    tx_kind                     SMALLINT     NOT NULL,
    PRIMARY KEY(tx_kind, tx_insertion_order)
);


-- Main table storing data about optimistically indexed events
-- (events produced by transactions that were executed by the indexer,
-- and indexed without waiting for the transaction to be checkpointed).
-- Equivalent of `events` table.
CREATE TABLE optimistic_events
(
    tx_insertion_order          BIGINT       REFERENCES optimistic_transactions(insertion_order) ON DELETE CASCADE,
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
    PRIMARY KEY(tx_insertion_order, event_sequence_number)
);
CREATE INDEX optimistic_events_package ON optimistic_events (package, tx_insertion_order, event_sequence_number);
CREATE INDEX optimistic_events_package_module ON optimistic_events (package, module, tx_insertion_order, event_sequence_number);
CREATE INDEX optimistic_events_event_type ON optimistic_events (event_type text_pattern_ops, tx_insertion_order, event_sequence_number);

-- Lookup table to search for optimistic events by emitting package address.
-- Equivalent of `event_emit_package` table.
CREATE TABLE optimistic_event_emit_package
(
    package                     BYTEA   NOT NULL,
    tx_insertion_order          BIGINT  REFERENCES optimistic_transactions(insertion_order) ON DELETE CASCADE,
    event_sequence_number       BIGINT  NOT NULL,
    sender                      BYTEA   NOT NULL,
    PRIMARY KEY(package, tx_insertion_order, event_sequence_number)
);
CREATE INDEX optimistic_event_emit_package_sender ON optimistic_event_emit_package (sender, package, tx_insertion_order, event_sequence_number);

-- Lookup table to search for optimistic events by emitting module name.
-- Equivalent of `event_emit_module` table.
CREATE TABLE optimistic_event_emit_module
(
    package                     BYTEA   NOT NULL,
    module                      TEXT    NOT NULL,
    tx_insertion_order          BIGINT  REFERENCES optimistic_transactions(insertion_order) ON DELETE CASCADE,
    event_sequence_number       BIGINT  NOT NULL,
    sender                      BYTEA   NOT NULL,
    PRIMARY KEY(package, module, tx_insertion_order, event_sequence_number)
);
CREATE INDEX optimistic_event_emit_module_sender ON optimistic_event_emit_module (sender, package, module, tx_insertion_order, event_sequence_number);

-- Lookup table to search for optimistic events by package address of emitted type.
-- Equivalent of `event_struct_package` table.
CREATE TABLE optimistic_event_struct_package
(
    package                     BYTEA   NOT NULL,
    tx_insertion_order          BIGINT  REFERENCES optimistic_transactions(insertion_order) ON DELETE CASCADE,
    event_sequence_number       BIGINT  NOT NULL,
    sender                      BYTEA   NOT NULL,
    PRIMARY KEY(package, tx_insertion_order, event_sequence_number)
);
CREATE INDEX optimistic_event_struct_package_sender ON optimistic_event_struct_package (sender, package, tx_insertion_order, event_sequence_number);

-- Lookup table to search for optimistic events by module name of emitted type.
-- Equivalent of `event_struct_module` table.
CREATE TABLE optimistic_event_struct_module
(
    package                     BYTEA   NOT NULL,
    module                      TEXT    NOT NULL,
    tx_insertion_order          BIGINT  REFERENCES optimistic_transactions(insertion_order) ON DELETE CASCADE,
    event_sequence_number       BIGINT  NOT NULL,
    sender                      BYTEA   NOT NULL,
    PRIMARY KEY(package, module, tx_insertion_order, event_sequence_number)
);
CREATE INDEX optimistic_event_struct_module_sender ON optimistic_event_struct_module (sender, package, module, tx_insertion_order, event_sequence_number);

-- Lookup table to search for optimistic events by emitted type name.
-- Equivalent of `event_struct_name` table.
CREATE TABLE optimistic_event_struct_name
(
    package                     BYTEA   NOT NULL,
    module                      TEXT    NOT NULL,
    type_name                   TEXT    NOT NULL,
    tx_insertion_order          BIGINT  REFERENCES optimistic_transactions(insertion_order) ON DELETE CASCADE,
    event_sequence_number       BIGINT  NOT NULL,
    sender                      BYTEA   NOT NULL,
    PRIMARY KEY(package, module, type_name, tx_insertion_order, event_sequence_number)
);
CREATE INDEX optimistic_event_struct_name_sender ON optimistic_event_struct_name (sender, package, module, type_name, tx_insertion_order, event_sequence_number);

-- Lookup table to search for optimistic events by emitted type name with type parameters.
-- Equivalent of `event_struct_instantiation` table.
CREATE TABLE optimistic_event_struct_instantiation
(
    package                     BYTEA   NOT NULL,
    module                      TEXT    NOT NULL,
    type_instantiation          TEXT    NOT NULL,
    tx_insertion_order          BIGINT  REFERENCES optimistic_transactions(insertion_order) ON DELETE CASCADE,
    event_sequence_number       BIGINT  NOT NULL,
    sender                      BYTEA   NOT NULL,
    PRIMARY KEY(package, module, type_instantiation, tx_insertion_order, event_sequence_number)
);
CREATE INDEX optimistic_event_struct_instantiation_sender ON optimistic_event_struct_instantiation (sender, package, module, type_instantiation, tx_insertion_order, event_sequence_number);

-- Lookup table to search for optimistic events by event sender address
-- Equivalent of `event_senders` table.
CREATE TABLE optimistic_event_senders
(
    sender                      BYTEA   NOT NULL,
    tx_insertion_order          BIGINT  REFERENCES optimistic_transactions(insertion_order) ON DELETE CASCADE,
    event_sequence_number       BIGINT  NOT NULL,
    PRIMARY KEY(sender, tx_insertion_order, event_sequence_number)
);
