-- Used for the exact instantiated type (e.g. 0x2::coin::Coin<0x2::iota::IOTA>).
CREATE INDEX CONCURRENTLY IF NOT EXISTS objects_backward_history_type_full
    ON objects_backward_history (object_type, superseded_at_checkpoint);
