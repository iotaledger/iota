-- `tx_global_order` replaces `tx_digests` as the digest-to-sequence-number
-- lookup table.
DROP TABLE IF EXISTS tx_digests;
DELETE FROM watermarks WHERE entity = 'tx_digests';

ALTER TABLE tx_global_order RENAME COLUMN chk_tx_sequence_number TO tx_sequence_number;
ALTER INDEX tx_global_order_chk_tx_seq_num RENAME TO tx_global_order_tx_seq_num;

-- `global_sequence_number` is no longer used for ordering; drop it from
-- `tx_global_order` together with its index. `optimistic_transactions`
-- keeps its copy as the pruning age signal.
DROP INDEX tx_global_order_seq_digest;
ALTER TABLE tx_global_order DROP COLUMN global_sequence_number;
