-- Restore the default per-column statistics target (-1 means
-- `default_statistics_target`, which is 100 by default).
ALTER TABLE checkpointed_objects
    ALTER COLUMN object_type_package SET STATISTICS -1;

ANALYZE checkpointed_objects (object_type_package);
