--- Your SQL goes here
DROP STATISTICS IF EXISTS objects_snapshot_type_stats;
DROP STATISTICS IF EXISTS objects_type_stats;

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
            'DROP STATISTICS IF EXISTS %I',
            partition_name || '_type_stats'
        );
    END LOOP;
END $$;

DROP STATISTICS IF EXISTS objects_history_type_stats;
