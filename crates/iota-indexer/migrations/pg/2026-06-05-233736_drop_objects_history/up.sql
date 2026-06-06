-- Drop objects_history table with CASCADE to clean up partition tables, indexes, and statistics
DROP TABLE IF EXISTS objects_history CASCADE;

-- Delete the watermark entry for objects_history
DELETE FROM watermarks WHERE entity = 'objects_history';
