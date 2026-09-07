-- The object_id-first unique index that becomes the new primary key. Built under
-- a temporary name so an object_id-first index always exists while the old
-- id_version index is dropped in the swap that follows.
-- (object_id, object_version) is unique - a version is superseded exactly once -
-- and superseded_at_checkpoint stays in the key so per-object lookups can read it
-- without touching the table.
CREATE UNIQUE INDEX CONCURRENTLY IF NOT EXISTS objects_backward_history_pk_new
    ON objects_backward_history (object_id, object_version, superseded_at_checkpoint);
