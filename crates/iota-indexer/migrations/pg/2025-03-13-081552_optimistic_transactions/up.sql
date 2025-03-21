-- Insertion order number that each transaction (either optimistic or
-- checkpointed) is assigned when being indexed. It provides common
-- ordering for optimistic and checkpointed transactions, whereas
-- `tx_sequence_number` provides ordering only for checkpointed transactions.
CREATE TABLE tx_insertion_order (
    tx_digest                   BYTEA        PRIMARY KEY,
    insertion_order             BIGSERIAL    NOT NULL
);
CREATE UNIQUE INDEX tx_insertion_order_insertion_order ON tx_insertion_order (insertion_order);

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
