-- Version of objects that already got deleted.
-- This is to handle race conditions between optimistic indexing and checkpoint indexing.
-- If object version is present in this table,
-- any inserts of lower versions for this object should be skipped.
-- Object version deleted by both optimistic and checkpoint indexing should be stored in this table.
CREATE TABLE optimistic_deleted_objects_versions (
    object_id      bytea   PRIMARY KEY,
    object_version bigint  NOT NULL
);
