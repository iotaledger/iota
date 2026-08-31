DELETE FROM watermarks WHERE entity = 'objects_snapshot';

-- CASCADE drops the indexes and extended statistics defined on the table
-- (objects_snapshot_type_stats).
DROP TABLE IF EXISTS objects_snapshot CASCADE;
