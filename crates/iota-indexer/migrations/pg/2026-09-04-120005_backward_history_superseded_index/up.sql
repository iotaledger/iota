-- Single-column index used to prune old checkpoints (the primary key did this
-- before the key was reordered to be object_id-first).
CREATE INDEX CONCURRENTLY IF NOT EXISTS objects_backward_history_superseded
    ON objects_backward_history (superseded_at_checkpoint);
