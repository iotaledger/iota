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
