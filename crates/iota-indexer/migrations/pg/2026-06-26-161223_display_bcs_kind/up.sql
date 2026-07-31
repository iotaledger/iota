-- Discriminates what the `bcs` column encodes:
--   0 = full DisplayVersionUpdatedEvent (legacy default)
--   1 = VecMap<String, String> display fields only
--       (this is what is used for rendering views)
--
-- We set initially to 0 to match the legacy encoding of existing databases.
ALTER TABLE display ADD COLUMN bcs_kind SMALLINT NOT NULL DEFAULT 0;

-- Restarting ingestion after this migration will encode the fields.
ALTER TABLE display ALTER COLUMN bcs_kind SET DEFAULT 1;
