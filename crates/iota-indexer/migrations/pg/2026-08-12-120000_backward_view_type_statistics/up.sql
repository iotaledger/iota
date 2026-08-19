-- Raise the extended-statistics target on the backward-diff tables so all
-- distinct object types fit in the multi-column MCV list; types outside the
-- list get a rows=1 estimate, leading to plans that time out.
ALTER STATISTICS checkpointed_objects_type_stats SET STATISTICS 2500;
ALTER STATISTICS objects_backward_history_type_stats SET STATISTICS 2500;

-- Rebuild the statistics objects modified above with their new target.
-- ANALYZE rebuilds them only if all their columns are listed; listing just
-- those columns keeps the ANALYZE cheap.
ANALYZE checkpointed_objects (object_type, object_type_package, object_type_module, object_type_name);
ANALYZE objects_backward_history (object_type, object_type_package, object_type_module, object_type_name);
