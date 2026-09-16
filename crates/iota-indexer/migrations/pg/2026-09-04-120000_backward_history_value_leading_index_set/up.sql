-- Rebuild the objects_backward_history indexes so each filter index starts with
-- its value column (owner / coin_type / object_type) instead of
-- superseded_at_checkpoint. A query on that value can then go straight to it
-- instead of reading the whole checkpoint window. Dropping object_id from these
-- indexes keeps them much smaller.
--
-- The primary key and id_version index also change - before:
--   pk         (superseded_at_checkpoint, object_id, object_version)
--   id_version (object_id, object_version, superseded_at_checkpoint)
-- after:
--   pk         (object_id, object_version, superseded_at_checkpoint)
--   superseded (superseded_at_checkpoint)
-- The object_id-first key also serves the per-object lookups id_version did, and
-- the single-column superseded index handles checkpoint pruning.
--
-- Runs in one transaction (indexes are built non-concurrently); this migration
-- does not run against a live database, so the table lock is acceptable.

-- Filter indexes: value column first, superseded_at_checkpoint last, no object_id.

-- owner_type 1 (address-owned) and 2 (object-owned, i.e. dynamic fields) are the
-- only owner kinds with an owner_id; immutable (0) and shared (3) have none.
CREATE INDEX IF NOT EXISTS objects_backward_history_owner_value
    ON objects_backward_history (owner_type, owner_id, superseded_at_checkpoint)
    WHERE owner_type >= 1 AND owner_type <= 2 AND owner_id IS NOT NULL;
-- coin_type is set only on coins (NULL for every other object).
CREATE INDEX IF NOT EXISTS objects_backward_history_coin_value
    ON objects_backward_history (coin_type, superseded_at_checkpoint)
    WHERE coin_type IS NOT NULL;
-- Address-owned coins only, for the per-address balance query.
CREATE INDEX IF NOT EXISTS objects_backward_history_coin_owner_value
    ON objects_backward_history (owner_id, coin_type, superseded_at_checkpoint)
    WHERE coin_type IS NOT NULL AND owner_type = 1;
-- Used for package/module/name filters (e.g. all coins, or a whole module).
CREATE INDEX IF NOT EXISTS objects_backward_history_type_generic
    ON objects_backward_history (object_type_package, object_type_module, object_type_name, superseded_at_checkpoint);
-- Used for the exact instantiated type (e.g. 0x2::coin::Coin<0x2::iota::IOTA>).
CREATE INDEX IF NOT EXISTS objects_backward_history_type_full
    ON objects_backward_history (object_type, superseded_at_checkpoint);
-- Single-column index used to prune old checkpoints (the pk did this before).
CREATE INDEX IF NOT EXISTS objects_backward_history_superseded
    ON objects_backward_history (superseded_at_checkpoint);

-- Swap the primary key to be object_id-first. (object_id, object_version) is
-- unique - a version is superseded exactly once - and superseded_at_checkpoint
-- stays in the key so per-object lookups can read it without touching the table.
ALTER TABLE objects_backward_history DROP CONSTRAINT IF EXISTS objects_backward_history_pk;
ALTER TABLE objects_backward_history
    ADD CONSTRAINT objects_backward_history_pk PRIMARY KEY (object_id, object_version, superseded_at_checkpoint);

-- Drop id_version (now covered by the pk) and the old superseded-leading indexes.
DROP INDEX IF EXISTS objects_backward_history_id_version;
DROP INDEX IF EXISTS objects_backward_history_owner;
DROP INDEX IF EXISTS objects_backward_history_owner_df;
DROP INDEX IF EXISTS objects_backward_history_owner_package_module_name_full_type;
DROP INDEX IF EXISTS objects_backward_history_package_module_name_full_type;
DROP INDEX IF EXISTS objects_backward_history_type;
DROP INDEX IF EXISTS objects_backward_history_coin_only;
DROP INDEX IF EXISTS objects_backward_history_coin_owner;
