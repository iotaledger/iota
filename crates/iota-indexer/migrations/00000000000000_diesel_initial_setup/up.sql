-- Diesel
CREATE OR REPLACE FUNCTION diesel_manage_updated_at(_tbl regclass) RETURNS VOID AS $$
BEGIN
    EXECUTE format('CREATE TRIGGER set_updated_at BEFORE UPDATE ON %s
                    FOR EACH ROW EXECUTE PROCEDURE diesel_set_updated_at()', _tbl);
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION diesel_set_updated_at() RETURNS trigger AS $$
BEGIN
    IF (
        NEW IS DISTINCT FROM OLD AND
        NEW.updated_at IS NOT DISTINCT FROM OLD.updated_at
    ) THEN
        NEW.updated_at := current_timestamp;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Events
CREATE TABLE events
(
    tx_sequence_number          BIGINT       NOT NULL,
    event_sequence_number       BIGINT       NOT NULL,
    transaction_digest          bytea        NOT NULL,
    checkpoint_sequence_number  bigint       NOT NULL,
    -- array of IotaAddress in bytes. All signers of the transaction.
    senders                     bytea[]      NOT NULL,
    -- bytes of the entry package ID
    package                     bytea        NOT NULL,
    -- entry module name
    module                      text         NOT NULL,
    -- StructTag in Display format
    event_type                  text         NOT NULL,
    timestamp_ms                BIGINT       NOT NULL,
    -- bcs of the Event contents (Event.contents)
    bcs                         BYTEA        NOT NULL,
    PRIMARY KEY(tx_sequence_number, event_sequence_number)
);

CREATE INDEX events_package ON events (package, tx_sequence_number, event_sequence_number);
CREATE INDEX events_package_module ON events (package, module, tx_sequence_number, event_sequence_number);
CREATE INDEX events_event_type ON events (event_type text_pattern_ops, tx_sequence_number, event_sequence_number);
CREATE INDEX events_checkpoint_sequence_number ON events (checkpoint_sequence_number);

-- Objects
CREATE TABLE objects (
    object_id                   bytea         PRIMARY KEY,
    object_version              bigint        NOT NULL,
    object_digest               bytea         NOT NULL,
    checkpoint_sequence_number  bigint        NOT NULL,
    -- Immutable/Address/Object/Shared, see types.rs
    owner_type                  smallint      NOT NULL,
    -- bytes of IotaAddress/ObjectID of the owner ID.
    -- Non-null for objects with an owner: Addresso or Objects
    owner_id                    bytea,
    -- Object type
    object_type                 text,
    -- bcs serialized Object
    serialized_object           bytea         NOT NULL,
    -- Non-null when the object is a coin.
    -- e.g. `0x2::iota::IOTA`
    coin_type                   text,
    -- Non-null when the object is a coin.
    coin_balance                bigint,
    -- DynamicField/DynamicObject, see types.rs
    -- Non-null when the object is a dynamic field
    df_kind                     smallint,
    -- bcs serialized DynamicFieldName
    -- Non-null when the object is a dynamic field
    df_name                     bytea,
    -- object_type in DynamicFieldInfo.
    df_object_type              text,
    -- object_id in DynamicFieldInfo.
    df_object_id                bytea
);

-- OwnerType: 1: Address, 2: Object, see types.rs
CREATE INDEX objects_owner ON objects (owner_type, owner_id) WHERE owner_type BETWEEN 1 AND 2 AND owner_id IS NOT NULL;
CREATE INDEX objects_coin ON objects (owner_id, coin_type) WHERE coin_type IS NOT NULL AND owner_type = 1;
CREATE INDEX objects_checkpoint_sequence_number ON objects (checkpoint_sequence_number);
CREATE INDEX objects_type ON objects (object_type);

-- Objects History
--
-- similar to objects table, except that
-- 1. the primary key to store multiple object versions and partitions by checkpoint_sequence_number
-- 2. allow null values in some columns for deleted / wrapped objects
-- 3. object_status to mark the status of the object, which is either Active or WrappedOrDeleted
CREATE TABLE objects_history (
    object_id                   bytea         NOT NULL,
    object_version              bigint        NOT NULL,
    object_status               smallint      NOT NULL,
    object_digest               bytea,
    checkpoint_sequence_number  bigint        NOT NULL,
    owner_type                  smallint,
    owner_id                    bytea,
    object_type                 text,
    serialized_object           bytea,
    coin_type                   text,
    coin_balance                bigint,
    df_kind                     smallint,
    df_name                     bytea,
    df_object_type              text,
    df_object_id                bytea,
    CONSTRAINT objects_history_pk PRIMARY KEY (checkpoint_sequence_number, object_id, object_version)
) PARTITION BY RANGE (checkpoint_sequence_number);
CREATE INDEX objects_history_owner ON objects_history (checkpoint_sequence_number, owner_type, owner_id) WHERE owner_type BETWEEN 1 AND 2 AND owner_id IS NOT NULL;
CREATE INDEX objects_history_coin ON objects_history (checkpoint_sequence_number, owner_id, coin_type) WHERE coin_type IS NOT NULL AND owner_type = 1;
CREATE INDEX objects_history_type ON objects_history (checkpoint_sequence_number, object_type);
-- init with first partition of the history table
CREATE TABLE objects_history_partition_0 PARTITION OF objects_history FOR VALUES FROM (0) TO (MAXVALUE);

-- Objects Snapshot
--
-- snapshot table by folding objects_history table until certain checkpoint,
-- effectively the snapshot of objects at the same checkpoint,
-- except that it also includes deleted or wrapped objects with the corresponding object_status.
CREATE TABLE objects_snapshot (
    object_id                   bytea         PRIMARY KEY,
    object_version              bigint        NOT NULL,
    object_status               smallint      NOT NULL,
    object_digest               bytea,
    checkpoint_sequence_number  bigint        NOT NULL,
    owner_type                  smallint,
    owner_id                    bytea,
    object_type                 text,
    serialized_object           bytea,
    coin_type                   text,
    coin_balance                bigint,
    df_kind                     smallint,
    df_name                     bytea,
    df_object_type              text,
    df_object_id                bytea
);
CREATE INDEX objects_snapshot_checkpoint_sequence_number ON objects_snapshot (checkpoint_sequence_number);
CREATE INDEX objects_snapshot_owner ON objects_snapshot (owner_type, owner_id, object_id) WHERE owner_type BETWEEN 1 AND 2 AND owner_id IS NOT NULL;
CREATE INDEX objects_snapshot_coin ON objects_snapshot (owner_id, coin_type, object_id) WHERE coin_type IS NOT NULL AND owner_type = 1;
CREATE INDEX objects_snapshot_type ON objects_snapshot (object_type, object_id);

-- Transactions
CREATE TABLE transactions (
    tx_sequence_number          BIGINT       NOT NULL,
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
    CONSTRAINT transactions_pkey PRIMARY KEY (tx_sequence_number, checkpoint_sequence_number)
) PARTITION BY RANGE (checkpoint_sequence_number);

CREATE TABLE transactions_partition_0 PARTITION OF transactions FOR VALUES FROM (0) TO (MAXVALUE);
CREATE INDEX transactions_transaction_digest ON transactions (transaction_digest);
CREATE INDEX transactions_checkpoint_sequence_number ON transactions (checkpoint_sequence_number);
-- only create index for system transactions (0). See types.rs
CREATE INDEX transactions_transaction_kind ON transactions (transaction_kind) WHERE transaction_kind = 0;

-- Checkpoints
CREATE TABLE checkpoints
(
    sequence_number                     bigint       PRIMARY KEY,
    checkpoint_digest                   bytea        NOT NULL,
    epoch                               bigint       NOT NULL,
    -- total transactions in the network at the end of this checkpoint (including itself)
    network_total_transactions          bigint       NOT NULL,
    previous_checkpoint_digest          bytea,
    -- if this checkpoitn is the last checkpoint of an epoch
    end_of_epoch                        boolean      NOT NULL,
    -- array of TranscationDigest in bytes included in this checkpoint
    tx_digests                          bytea[]      NOT NULL,
    timestamp_ms                        BIGINT       NOT NULL,
    total_gas_cost                      BIGINT       NOT NULL,
    computation_cost                    BIGINT       NOT NULL,
    storage_cost                        BIGINT       NOT NULL,
    storage_rebate                      BIGINT       NOT NULL,
    non_refundable_storage_fee          BIGINT       NOT NULL,
    -- bcs serialized Vec<CheckpointCommitment> bytes
    checkpoint_commitments              bytea        NOT NULL,
    -- bcs serialized AggregateAuthoritySignature bytes
    validator_signature                 bytea        NOT NULL,
    -- bcs serialzied EndOfEpochData bytes, if the checkpoint marks end of an epoch
    end_of_epoch_data                   bytea
);

CREATE INDEX checkpoints_epoch ON checkpoints (epoch, sequence_number);
CREATE INDEX checkpoints_digest ON checkpoints USING HASH (checkpoint_digest);

-- Epochs
CREATE TABLE epochs
(
    epoch                           BIGINT      PRIMARY KEY,
    first_checkpoint_id             BIGINT      NOT NULL,
    epoch_start_timestamp           BIGINT      NOT NULL,
    reference_gas_price             BIGINT      NOT NULL,
    protocol_version                BIGINT      NOT NULL,
    total_stake                     BIGINT      NOT NULL,
    storage_fund_balance            BIGINT      NOT NULL,
    system_state                    bytea       NOT NULL,
    -- The following fields are nullable because they are filled in
    -- only at the end of an epoch.
    epoch_total_transactions        BIGINT,
    last_checkpoint_id              BIGINT,
    epoch_end_timestamp             BIGINT,
    -- The following fields are from SystemEpochInfoEvent emitted
    -- **after** advancing to the next epoch
    storage_charge                  BIGINT,
    storage_rebate                  BIGINT,
    total_gas_fees                  BIGINT,
    total_stake_rewards_distributed BIGINT,
    -- bcs serialized Vec<EpochCommitment> bytes, found in last CheckpointSummary
    -- of the epoch
    epoch_commitments               bytea,
    burnt_tokens_amount             BIGINT,
    minted_tokens_amount            BIGINT
);

-- Packages
CREATE TABLE packages
(
    package_id                   bytea  PRIMARY KEY,
    -- bcs serialized MovePackage
    move_package                 bytea  NOT NULL
);

-- Tx Recipients
CREATE TABLE tx_recipients (
    tx_sequence_number          BIGINT       NOT NULL,
    -- IotaAddress in bytes.
    recipient                   BYTEA        NOT NULL,
    PRIMARY KEY(recipient, tx_sequence_number)
);
CREATE INDEX tx_recipients_tx_sequence_number_index ON tx_recipients (tx_sequence_number ASC);

-- Tx Senders
CREATE TABLE tx_senders (
    tx_sequence_number          BIGINT       NOT NULL,
    -- IotaAddress in bytes.
    sender                      BYTEA        NOT NULL,
    PRIMARY KEY(sender, tx_sequence_number)
);
CREATE INDEX tx_senders_tx_sequence_number_index ON tx_senders (tx_sequence_number ASC);

-- Tx Input Objects
CREATE TABLE tx_input_objects (
    tx_sequence_number          BIGINT       NOT NULL,
    -- Object ID in bytes.
    object_id                   BYTEA        NOT NULL,
    PRIMARY KEY(object_id, tx_sequence_number)
);

-- Tx changed Objects
CREATE TABLE tx_changed_objects (
    tx_sequence_number          BIGINT       NOT NULL,
    -- Object Id in bytes.
    object_id                   BYTEA        NOT NULL,
    PRIMARY KEY(object_id, tx_sequence_number)
);

-- Tx Calls
CREATE TABLE tx_calls (
    tx_sequence_number          BIGINT       NOT NULL,
    package                     BYTEA        NOT NULL,
    module                      TEXT         NOT NULL,
    func                        TEXT         NOT NULL,
    -- 1. Using Primary Key as a unique index.
    -- 2. Diesel does not like tables with no primary key.
    PRIMARY KEY(package, tx_sequence_number)
);

CREATE INDEX tx_calls_module ON tx_calls (package, module, tx_sequence_number);
CREATE INDEX tx_calls_func ON tx_calls (package, module, func, tx_sequence_number);
CREATE INDEX tx_calls_tx_sequence_number ON tx_calls (tx_sequence_number);

-- Display
CREATE TABLE display
(
    object_type     text        PRIMARY KEY,
    id              BYTEA       NOT NULL,
    version         SMALLINT    NOT NULL,
    bcs             BYTEA       NOT NULL
);

CREATE OR REPLACE FUNCTION query_cost(
    query_in text,
    cost OUT float8
 ) RETURNS float8  AS
$$DECLARE
 p json;
BEGIN
    /* get execution plan in JSON */
    EXECUTE 'EXPLAIN (FORMAT JSON) ' || query_in INTO p;
    /* extract total cost */
    SELECT p->0->'Plan'->>'Total Cost'
        INTO cost;
    RETURN;
END;$$ LANGUAGE plpgsql STRICT;

CREATE OR REPLACE PROCEDURE advance_partition(table_name TEXT, last_epoch BIGINT, new_epoch BIGINT, last_epoch_start_cp BIGINT, new_epoch_start_cp BIGINT)
LANGUAGE plpgsql
AS $$
BEGIN
    EXECUTE format('ALTER TABLE %I DETACH PARTITION %I_partition_%s', table_name, table_name, last_epoch);
    EXECUTE format('ALTER TABLE %I ATTACH PARTITION %I_partition_%s FOR VALUES FROM (%L) TO (%L)', table_name, table_name, last_epoch, last_epoch_start_cp, new_epoch_start_cp);
    EXECUTE format('CREATE TABLE IF NOT EXISTS %I_partition_%s PARTITION OF %I FOR VALUES FROM (%L) TO (MAXVALUE)', table_name, new_epoch, table_name, new_epoch_start_cp);
END;
$$;