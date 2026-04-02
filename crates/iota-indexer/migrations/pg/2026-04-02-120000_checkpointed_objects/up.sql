-- Checkpointed objects table: exact copy of the objects table, written only
-- by the checkpoint ingestion pipeline. Used as the base table for backward
-- diff consistent views, avoiding race conditions with optimistic indexing
-- which writes to the regular objects table.
CREATE TABLE checkpointed_objects (
    object_id                   bytea         PRIMARY KEY,
    object_version              bigint        NOT NULL,
    object_digest               bytea         NOT NULL,
    owner_type                  smallint      NOT NULL,
    owner_id                    bytea,
    object_type                 text,
    object_type_package         bytea,
    object_type_module          text,
    object_type_name            text,
    serialized_object           bytea         NOT NULL,
    coin_type                   text,
    coin_balance                bigint,
    df_kind                     smallint
);

-- Indexes mirror the objects table
CREATE INDEX checkpointed_objects_owner ON checkpointed_objects (owner_type, owner_id) WHERE owner_type BETWEEN 1 AND 2 AND owner_id IS NOT NULL;
CREATE INDEX checkpointed_objects_owner_object_id ON checkpointed_objects (owner_type, owner_id, object_id) WHERE owner_type BETWEEN 1 AND 2 AND owner_id IS NOT NULL;
CREATE INDEX checkpointed_objects_coin ON checkpointed_objects (owner_id, coin_type) WHERE coin_type IS NOT NULL AND owner_type = 1;
CREATE INDEX checkpointed_objects_package_module_name_full_type ON checkpointed_objects (object_type_package, object_type_module, object_type_name, object_type);
CREATE INDEX checkpointed_objects_owner_package_module_name_full_type ON checkpointed_objects (owner_id, object_type_package, object_type_module, object_type_name, object_type);
CREATE STATISTICS checkpointed_objects_type_stats (dependencies, mcv) ON object_type, object_type_package, object_type_module, object_type_name FROM checkpointed_objects;
