-- Recreate the objects_history table in the state it had just before this
-- migration

CREATE TABLE objects_history (
    object_id                   bytea         NOT NULL,
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
    df_kind                     smallint,
    CONSTRAINT objects_history_pk PRIMARY KEY (checkpoint_sequence_number, object_id, object_version)
) PARTITION BY RANGE (checkpoint_sequence_number);
CREATE INDEX objects_history_id_version ON objects_history (object_id, object_version, checkpoint_sequence_number);
CREATE INDEX objects_history_owner ON objects_history (checkpoint_sequence_number, owner_type, owner_id) WHERE owner_type BETWEEN 1 AND 2 AND owner_id IS NOT NULL;
CREATE INDEX objects_history_coin_owner ON objects_history (checkpoint_sequence_number, owner_id, coin_type, object_id) WHERE coin_type IS NOT NULL AND owner_type = 1;
CREATE INDEX objects_history_coin_only ON objects_history (checkpoint_sequence_number, coin_type, object_id) WHERE coin_type IS NOT NULL;
CREATE INDEX objects_history_type ON objects_history (checkpoint_sequence_number, object_type);
CREATE INDEX objects_history_package_module_name_full_type ON objects_history (checkpoint_sequence_number, object_type_package, object_type_module, object_type_name, object_type);
CREATE INDEX objects_history_owner_package_module_name_full_type ON objects_history (checkpoint_sequence_number, owner_id, object_type_package, object_type_module, object_type_name, object_type);
CREATE TABLE objects_history_partition_0 PARTITION OF objects_history FOR VALUES FROM (0) TO (MAXVALUE);

CREATE STATISTICS IF NOT EXISTS objects_history_type_stats (dependencies, mcv)
ON object_type_package, object_type_module, object_type_name, object_type
FROM objects_history;

CREATE STATISTICS IF NOT EXISTS objects_history_partition_0_type_stats (dependencies, mcv)
ON object_type_package, object_type_module, object_type_name, object_type
FROM objects_history_partition_0;
