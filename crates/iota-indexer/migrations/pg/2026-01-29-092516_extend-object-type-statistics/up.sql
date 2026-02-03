-- We introduce additional statistics to help the planner use better estimates
-- on closely correlated columns.
--
-- See https://www.postgresql.org/docs/current/planner-stats.html#PLANNER-STATS-EXTENDED
CREATE STATISTICS IF NOT EXISTS objects_snapshot_type_stats (dependencies, mcv)
ON object_type_package, object_type_module, object_type_name, object_type
FROM objects_snapshot;

CREATE STATISTICS IF NOT EXISTS objects_type_stats (dependencies, mcv)
ON object_type_package, object_type_module, object_type_name, object_type
FROM objects;

CREATE STATISTICS IF NOT EXISTS objects_history_type_stats (dependencies, mcv)
ON object_type_package, object_type_module, object_type_name, object_type
FROM objects_history;

-- Extend statistics for each partition
DO $$
DECLARE
    partition_name TEXT;
BEGIN
    FOR partition_name IN
        SELECT tablename
        FROM pg_tables
        WHERE schemaname = 'public'
          AND tablename LIKE 'objects_history_%'
    LOOP
        EXECUTE format(
            'CREATE STATISTICS IF NOT EXISTS %I (dependencies, mcv)
             ON object_type_package, object_type_module, object_type_name, object_type
             FROM %I',
            partition_name || '_type_stats',
            partition_name
        );
    END LOOP;
END $$;

-- Initialize the statistics
ANALYZE objects (object_type_package, object_type_module, object_type_name, object_type),
        objects_snapshot(object_type_package, object_type_module, object_type_name, object_type),
        objects_history(object_type_package, object_type_module, object_type_name, object_type);
