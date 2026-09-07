-- Restore the superseded-leading indexes and primary key. Runs in one
-- transaction, so the indexes are rebuilt non-concurrently (this holds a lock
-- for the duration - acceptable for a rollback).
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
CREATE UNIQUE INDEX IF NOT EXISTS objects_backward_history_pk_orig
    ON objects_backward_history (superseded_at_checkpoint, object_id, object_version);
ALTER TABLE objects_backward_history DROP CONSTRAINT IF EXISTS objects_backward_history_pk;
ALTER TABLE objects_backward_history
    ADD CONSTRAINT objects_backward_history_pk PRIMARY KEY USING INDEX objects_backward_history_pk_orig;
CREATE INDEX IF NOT EXISTS objects_backward_history_id_version
    ON objects_backward_history (object_id, object_version, superseded_at_checkpoint);
