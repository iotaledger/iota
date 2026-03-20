-- Stores the checkpoint sequence number at which this object was, or will be, indexed.
-- Readers can consider the object finalized when this checkpoint has been
-- marked as fully indexed.
--
-- NULL means the object is already finalized (default for existing objects
-- and objects written by the optimistic path).
ALTER TABLE objects ADD COLUMN finalized_in_cp bigint DEFAULT NULL;
