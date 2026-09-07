-- Make the object_id-first index the primary key and drop the old
-- superseded-leading indexes. Before / after:
--   pk         (superseded_at_checkpoint, object_id, object_version)
--            -> (object_id, object_version, superseded_at_checkpoint)
--   id_version (object_id, object_version, superseded_at_checkpoint) -> dropped
--              (the new pk also serves the per-object lookups it used to)
--   pruning: single-column superseded index (added earlier) instead of the pk
--
-- Runs in one transaction: swapping the primary key already takes a brief
-- ACCESS EXCLUSIVE lock, and dropping indexes is a fast catalog change, so they
-- share that lock. ADD CONSTRAINT ... USING INDEX renames pk_new to
-- objects_backward_history_pk.
ALTER TABLE objects_backward_history DROP CONSTRAINT IF EXISTS objects_backward_history_pk;
ALTER TABLE objects_backward_history
    ADD CONSTRAINT objects_backward_history_pk PRIMARY KEY USING INDEX objects_backward_history_pk_new;
DROP INDEX IF EXISTS objects_backward_history_id_version;
DROP INDEX IF EXISTS objects_backward_history_owner;
DROP INDEX IF EXISTS objects_backward_history_owner_df;
DROP INDEX IF EXISTS objects_backward_history_owner_package_module_name_full_type;
DROP INDEX IF EXISTS objects_backward_history_package_module_name_full_type;
DROP INDEX IF EXISTS objects_backward_history_type;
DROP INDEX IF EXISTS objects_backward_history_coin_only;
DROP INDEX IF EXISTS objects_backward_history_coin_owner;
