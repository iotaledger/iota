-- This file should undo anything in `up.sql`
CREATE INDEX CONCURRENTLY IF NOT EXISTS objects_owner ON objects (owner_type, owner_id) WHERE owner_type BETWEEN 1 AND 2 AND owner_id IS NOT NULL;
