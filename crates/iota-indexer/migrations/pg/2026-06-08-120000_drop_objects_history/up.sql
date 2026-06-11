DELETE FROM watermarks WHERE entity = 'objects_history';

-- CASCADE drops all partitions, indexes, and extended statistics defined on
-- the table (objects_history_type_stats and the per-partition *_type_stats).
DROP TABLE IF EXISTS objects_history CASCADE;
