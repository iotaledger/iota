-- Address-owned coins only, for the per-address balance query.
CREATE INDEX CONCURRENTLY IF NOT EXISTS objects_backward_history_coin_owner_value
    ON objects_backward_history (owner_id, coin_type, superseded_at_checkpoint)
    WHERE coin_type IS NOT NULL AND owner_type = 1;
