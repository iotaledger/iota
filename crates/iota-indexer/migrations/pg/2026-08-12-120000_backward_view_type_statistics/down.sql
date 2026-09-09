-- Restore the default extended-statistics target.
ALTER STATISTICS checkpointed_objects_type_stats SET STATISTICS -1;
ALTER STATISTICS objects_backward_history_type_stats SET STATISTICS -1;

ANALYZE checkpointed_objects (object_type, object_type_package, object_type_module, object_type_name);
ANALYZE objects_backward_history (object_type, object_type_package, object_type_module, object_type_name);
