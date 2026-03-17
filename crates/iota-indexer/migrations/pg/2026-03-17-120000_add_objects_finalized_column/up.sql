-- Indicates that neither checkpoint nor optimistic indexing will write this
-- object at its current version again, so follow-up operations (e.g. updates,
-- deletions) can safely proceed without risk of being overwritten by concurrent
-- indexing.
--
-- This flag is only an indicator, not a protection mechanism. The actual write
-- protection is enforced by the tx status in `tx_global_order`.
--
-- Default is true because all existing objects are already finalized.
ALTER TABLE objects ADD COLUMN finalized bool NOT NULL DEFAULT true;
