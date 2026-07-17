DROP TABLE IF EXISTS objects_backward_history;
DELETE FROM watermarks WHERE entity = 'objects_backward_history';
