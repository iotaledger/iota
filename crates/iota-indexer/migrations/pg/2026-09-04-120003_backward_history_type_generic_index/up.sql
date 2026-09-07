-- Used for package/module/name filters (e.g. all coins, or a whole module).
CREATE INDEX CONCURRENTLY IF NOT EXISTS objects_backward_history_type_generic
    ON objects_backward_history (object_type_package, object_type_module, object_type_name, superseded_at_checkpoint);
