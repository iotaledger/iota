-- Checkpointed objects table: mirrors objects_snapshot schema, written only
-- by the checkpoint ingestion pipeline. Used as the base table for backward
-- diff consistent views, avoiding race conditions with optimistic indexing
-- which writes to the regular objects table.
-- Unlike the plain objects table, this also includes deleted or wrapped objects
-- with the corresponding object_status (matching objects_snapshot behaviour).

-- Create as UNLOGGED to skip WAL during bulk load and index creation.
CREATE UNLOGGED TABLE checkpointed_objects (
    object_id                   bytea         PRIMARY KEY,
    object_version              bigint        NOT NULL,
    object_status               smallint      NOT NULL,
    object_digest               bytea,
    checkpoint_sequence_number  bigint        NOT NULL,
    owner_type                  smallint,
    owner_id                    bytea,
    object_type                 text,
    object_type_package         bytea,
    object_type_module          text,
    object_type_name            text,
    serialized_object           bytea,
    coin_type                   text,
    coin_balance                bigint,
    df_kind                     smallint
);

-- Step 1: Bulk load from objects_snapshot (includes both active and
-- wrapped/deleted objects). The snapshot lags behind the latest checkpoint
-- by a configurable threshold (default 300).
INSERT INTO checkpointed_objects (
    object_id, object_version, object_status, object_digest,
    checkpoint_sequence_number, owner_type, owner_id,
    object_type, object_type_package, object_type_module, object_type_name,
    serialized_object, coin_type, coin_balance, df_kind
)
SELECT
    object_id, object_version, object_status, object_digest,
    checkpoint_sequence_number, owner_type, owner_id,
    object_type, object_type_package, object_type_module, object_type_name,
    serialized_object, coin_type, coin_balance, df_kind
FROM objects_snapshot;

-- Step 2: Apply the most recent changes from objects_history to bring the
-- table up to date (covering the gap between the snapshot checkpoint and
-- the latest ingested checkpoint).
INSERT INTO checkpointed_objects (
    object_id, object_version, object_status, object_digest,
    checkpoint_sequence_number, owner_type, owner_id,
    object_type, object_type_package, object_type_module, object_type_name,
    serialized_object, coin_type, coin_balance, df_kind
)
SELECT DISTINCT ON (object_id)
    object_id, object_version, object_status, object_digest,
    checkpoint_sequence_number, owner_type, owner_id,
    object_type, object_type_package, object_type_module, object_type_name,
    serialized_object, coin_type, coin_balance, df_kind
FROM objects_history
-- Use the watermarks table to get the last fully committed snapshot checkpoint.
-- Unlike MAX(checkpoint_sequence_number) from objects_snapshot data, the watermark
-- is only updated after all parallel chunks of a batch commit successfully,
-- so it is safe to use as the boundary even after a crash mid-batch.
-- The ON CONFLICT DO UPDATE makes any overlap with snapshot data idempotent.
WHERE checkpoint_sequence_number >= COALESCE(
    (SELECT max_committed_cp FROM watermarks WHERE entity = 'objects_snapshot'), 0
)
ORDER BY object_id, checkpoint_sequence_number DESC, object_version DESC
ON CONFLICT (object_id) DO UPDATE SET
    object_version              = EXCLUDED.object_version,
    object_status               = EXCLUDED.object_status,
    object_digest               = EXCLUDED.object_digest,
    checkpoint_sequence_number  = EXCLUDED.checkpoint_sequence_number,
    owner_type                  = EXCLUDED.owner_type,
    owner_id                    = EXCLUDED.owner_id,
    object_type                 = EXCLUDED.object_type,
    object_type_package         = EXCLUDED.object_type_package,
    object_type_module          = EXCLUDED.object_type_module,
    object_type_name            = EXCLUDED.object_type_name,
    serialized_object           = EXCLUDED.serialized_object,
    coin_type                   = EXCLUDED.coin_type,
    coin_balance                = EXCLUDED.coin_balance,
    df_kind                     = EXCLUDED.df_kind;

-- Build indexes (still unlogged, so index creation is also faster).
-- Mirrors objects_snapshot indexes.
CREATE INDEX checkpointed_objects_checkpoint_sequence_number ON checkpointed_objects (checkpoint_sequence_number);
CREATE INDEX checkpointed_objects_owner ON checkpointed_objects (owner_type, owner_id, object_id) WHERE owner_type BETWEEN 1 AND 2 AND owner_id IS NOT NULL;
CREATE INDEX checkpointed_objects_coin_owner ON checkpointed_objects (owner_id, coin_type, object_id) WHERE coin_type IS NOT NULL AND owner_type = 1;
CREATE INDEX checkpointed_objects_coin_only ON checkpointed_objects (coin_type, object_id) WHERE coin_type IS NOT NULL;
CREATE INDEX checkpointed_objects_id_type ON checkpointed_objects (object_id, object_type_package, object_type_module, object_type_name, object_type);
CREATE INDEX checkpointed_objects_type_id ON checkpointed_objects (object_type_package, object_type_module, object_type_name, object_type, object_id);
CREATE INDEX checkpointed_objects_owner_package_module_name_full_type ON checkpointed_objects (owner_id, object_type_package, object_type_module, object_type_name, object_type);
CREATE STATISTICS checkpointed_objects_type_stats (dependencies, mcv) ON object_type, object_type_package, object_type_module, object_type_name FROM checkpointed_objects;

-- Make the table durable.
ALTER TABLE checkpointed_objects SET LOGGED;
