CREATE INDEX CONCURRENTLY IF NOT EXISTS checkpointed_objects_id_type ON checkpointed_objects (object_id, object_type_package, object_type_module, object_type_name, object_type);
