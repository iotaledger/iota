-- Value-leading filter index: owner columns first, superseded_at_checkpoint last.
-- A query on the owner can go straight to it instead of reading the whole
-- checkpoint window; dropping object_id keeps the index small.
--
-- owner_type 1 (address-owned) and 2 (object-owned, i.e. dynamic fields) are the
-- only owner kinds with an owner_id; immutable (0) and shared (3) have none.
CREATE INDEX CONCURRENTLY IF NOT EXISTS objects_backward_history_owner_value
    ON objects_backward_history (owner_type, owner_id, superseded_at_checkpoint)
    WHERE owner_type >= 1 AND owner_type <= 2 AND owner_id IS NOT NULL;
