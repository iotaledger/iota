-- Bump the per-column statistics target for `object_type_package` so the
-- planner gets accurate row-count estimates for unpopular packages. This is
-- required for type-filtered `objects` GraphQL queries to pick the right
-- index.
--
-- See https://github.com/iotaledger/iota/issues/11711
ALTER TABLE checkpointed_objects
    ALTER COLUMN object_type_package SET STATISTICS 1000;

ANALYZE checkpointed_objects (object_type_package);
