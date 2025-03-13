CREATE TABLE tx_insertion_order (
    tx_digest                   BYTEA        PRIMARY KEY,
    insertion_order             BIGSERIAL    NOT NULL
);
CREATE UNIQUE INDEX tx_insertion_order_insertion_order ON tx_insertion_order (insertion_order);

CREATE TABLE optimistic_transactions (
    insertion_order             BIGINT       REFERENCES tx_insertion_order(insertion_order) ON DELETE CASCADE,
    transaction_digest          bytea        NOT NULL,
    -- bcs serialized SenderSignedData bytes
    raw_transaction             bytea        NOT NULL,
    -- bcs serialized TransactionEffects bytes
    raw_effects                 bytea        NOT NULL,
    checkpoint_sequence_number  BIGINT       NOT NULL,
    timestamp_ms                BIGINT       NOT NULL,
    -- array of bcs serialized IndexedObjectChange bytes
    object_changes              bytea[]      NOT NULL,
    -- array of bcs serialized BalanceChange bytes
    balance_changes             bytea[]      NOT NULL,
    -- array of bcs serialized StoredEvent bytes
    events                      bytea[]      NOT NULL,
    -- SystemTransaction/ProgrammableTransaction. See types.rs
    transaction_kind            smallint     NOT NULL,
    -- number of successful commands in this transaction, bound by number of command
    -- in a programmaable transaction.
    success_command_count       smallint     NOT NULL,
    PRIMARY KEY (insertion_order)
);

CREATE TABLE optimistic_tx_senders (
    tx_insertion_order          BIGINT       REFERENCES tx_insertion_order(insertion_order) ON DELETE CASCADE,
    sender                      BYTEA        NOT NULL,
    PRIMARY KEY(sender, tx_insertion_order)
);

CREATE TABLE optimistic_tx_recipients (
    tx_insertion_order          BIGINT       REFERENCES tx_insertion_order(insertion_order) ON DELETE CASCADE,
    recipient                   BYTEA        NOT NULL,
    sender                      BYTEA        NOT NULL,
    PRIMARY KEY(recipient, tx_insertion_order)
);
CREATE INDEX optimistic_tx_recipients_sender ON optimistic_tx_recipients (sender, recipient, tx_insertion_order);

CREATE TABLE optimistic_tx_input_objects (
    tx_insertion_order          BIGINT       REFERENCES tx_insertion_order(insertion_order) ON DELETE CASCADE,
    object_id                   BYTEA        NOT NULL,
    sender                      BYTEA        NOT NULL,
    PRIMARY KEY(object_id, tx_insertion_order)
);
CREATE INDEX optimistic_tx_input_objects_tx_insertion_order_index ON optimistic_tx_input_objects (tx_insertion_order);
CREATE INDEX optimistic_tx_input_objects_sender ON optimistic_tx_input_objects (sender, object_id, tx_insertion_order);

CREATE TABLE optimistic_tx_changed_objects (
    tx_insertion_order          BIGINT       REFERENCES tx_insertion_order(insertion_order) ON DELETE CASCADE,
    object_id                   BYTEA        NOT NULL,
    sender                      BYTEA        NOT NULL,
    PRIMARY KEY(object_id, tx_insertion_order)
);
CREATE INDEX optimistic_tx_changed_objects_tx_insertion_order_index ON optimistic_tx_changed_objects (tx_insertion_order);
CREATE INDEX optimistic_tx_changed_objects_sender ON optimistic_tx_changed_objects (sender, object_id, tx_insertion_order);

CREATE TABLE optimistic_tx_calls_pkg (
    tx_insertion_order          BIGINT       REFERENCES tx_insertion_order(insertion_order) ON DELETE CASCADE,
    package                     BYTEA        NOT NULL,
    sender                      BYTEA        NOT NULL,
    PRIMARY KEY(package, tx_insertion_order)
);
CREATE INDEX optimistic_tx_calls_pkg_sender ON optimistic_tx_calls_pkg (sender, package, tx_insertion_order);

CREATE TABLE optimistic_tx_calls_mod (
    tx_insertion_order          BIGINT       REFERENCES tx_insertion_order(insertion_order) ON DELETE CASCADE,
    package                     BYTEA        NOT NULL,
    module                      TEXT         NOT NULL,
    sender                      BYTEA        NOT NULL,
    PRIMARY KEY(package, module, tx_insertion_order)
);
CREATE INDEX optimistic_tx_calls_mod_sender ON optimistic_tx_calls_mod (sender, package, module, tx_insertion_order);

CREATE TABLE optimistic_tx_calls_fun (
    tx_insertion_order          BIGINT       REFERENCES tx_insertion_order(insertion_order) ON DELETE CASCADE,
    package                     BYTEA        NOT NULL,
    module                      TEXT         NOT NULL,
    func                        TEXT         NOT NULL,
    sender                      BYTEA        NOT NULL,
    PRIMARY KEY(package, module, func, tx_insertion_order)
);
CREATE INDEX optimistic_tx_calls_fun_sender ON optimistic_tx_calls_fun (sender, package, module, func, tx_insertion_order);

CREATE TABLE optimistic_tx_kinds (
    tx_insertion_order          BIGINT       REFERENCES tx_insertion_order(insertion_order) ON DELETE CASCADE,
    tx_kind                     SMALLINT     NOT NULL,
    PRIMARY KEY(tx_kind, tx_insertion_order)
);
