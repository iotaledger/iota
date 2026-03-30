CREATE INDEX CONCURRENTLY IF NOT EXISTS objects_owner_object_id ON objects (owner_type, owner_id, object_id) WHERE owner_type BETWEEN 1 AND 2 AND owner_id IS NOT NULL;
