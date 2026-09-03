-- Recreate the objects_snapshot table in the state it had just before this
-- migration

CREATE TABLE objects_snapshot (
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
CREATE INDEX objects_snapshot_checkpoint_sequence_number ON objects_snapshot (checkpoint_sequence_number);
CREATE INDEX objects_snapshot_owner ON objects_snapshot (owner_type, owner_id, object_id) WHERE owner_type BETWEEN 1 AND 2 AND owner_id IS NOT NULL;
CREATE INDEX objects_snapshot_coin_owner ON objects_snapshot (owner_id, coin_type, object_id) WHERE coin_type IS NOT NULL AND owner_type = 1;
CREATE INDEX objects_snapshot_coin_only ON objects_snapshot (coin_type, object_id) WHERE coin_type IS NOT NULL;
CREATE INDEX objects_snapshot_type_id ON objects_snapshot (object_type_package, object_type_module, object_type_name, object_type, object_id);
CREATE INDEX objects_snapshot_id_type ON objects_snapshot (object_id, object_type_package, object_type_module, object_type_name, object_type);
CREATE INDEX objects_snapshot_owner_package_module_name_full_type ON objects_snapshot (owner_id, object_type_package, object_type_module, object_type_name, object_type);

CREATE STATISTICS IF NOT EXISTS objects_snapshot_type_stats (dependencies, mcv)
ON object_type_package, object_type_module, object_type_name, object_type
FROM objects_snapshot;
