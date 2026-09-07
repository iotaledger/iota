-- coin_type is set only on coins (NULL for every other object).
CREATE INDEX CONCURRENTLY IF NOT EXISTS objects_backward_history_coin_value
    ON objects_backward_history (coin_type, superseded_at_checkpoint)
    WHERE coin_type IS NOT NULL;
