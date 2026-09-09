-- Recreate the original superseded-leading indexes.
CREATE INDEX IF NOT EXISTS objects_backward_history_owner
    ON objects_backward_history (superseded_at_checkpoint, owner_type, owner_id)
    WHERE owner_type >= 1 AND owner_type <= 2 AND owner_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS objects_backward_history_owner_df
    ON objects_backward_history (owner_id, object_id)
    WHERE owner_type = 2 AND df_kind IS NOT NULL;
CREATE INDEX IF NOT EXISTS objects_backward_history_owner_package_module_name_full_type
    ON objects_backward_history (superseded_at_checkpoint, owner_id, object_type_package, object_type_module, object_type_name, object_type);
CREATE INDEX IF NOT EXISTS objects_backward_history_package_module_name_full_type
    ON objects_backward_history (superseded_at_checkpoint, object_type_package, object_type_module, object_type_name, object_type);
CREATE INDEX IF NOT EXISTS objects_backward_history_type
    ON objects_backward_history (superseded_at_checkpoint, object_type);
CREATE INDEX IF NOT EXISTS objects_backward_history_coin_only
    ON objects_backward_history (superseded_at_checkpoint, coin_type, object_id)
    WHERE coin_type IS NOT NULL;
CREATE INDEX IF NOT EXISTS objects_backward_history_coin_owner
    ON objects_backward_history (superseded_at_checkpoint, owner_id, coin_type, object_id)
    WHERE coin_type IS NOT NULL AND owner_type = 1;

-- Restore the original superseded-leading primary key and the id_version index.
ALTER TABLE objects_backward_history DROP CONSTRAINT IF EXISTS objects_backward_history_pk;
ALTER TABLE objects_backward_history
    ADD CONSTRAINT objects_backward_history_pk PRIMARY KEY (superseded_at_checkpoint, object_id, object_version);
CREATE INDEX IF NOT EXISTS objects_backward_history_id_version
    ON objects_backward_history (object_id, object_version, superseded_at_checkpoint);

-- Drop the value-leading indexes.
DROP INDEX IF EXISTS objects_backward_history_owner_value;
DROP INDEX IF EXISTS objects_backward_history_coin_value;
DROP INDEX IF EXISTS objects_backward_history_coin_owner_value;
DROP INDEX IF EXISTS objects_backward_history_type_generic;
DROP INDEX IF EXISTS objects_backward_history_type_full;
DROP INDEX IF EXISTS objects_backward_history_superseded;
