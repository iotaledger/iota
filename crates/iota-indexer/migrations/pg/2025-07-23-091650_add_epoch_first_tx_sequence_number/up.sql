ALTER TABLE epochs ADD COLUMN first_tx_sequence_number bigint;

-- Use `epoch_total_transactins` to backfill the column for
-- all epochs but the current.
WITH epoch_first_tx_seq_num AS (
    SELECT epoch,
           SUM(epoch_total_transactions) OVER (ORDER BY epoch) - epoch_total_transactions AS first_tx_sequence_number
           FROM epochs
)
UPDATE epochs
SET first_tx_sequence_number = epoch_first_tx_seq_num.first_tx_sequence_number
FROM epoch_first_tx_seq_num
WHERE epochs.epoch = epoch_first_tx_seq_num.epoch;

-- Backfill the column for the current epoch.
UPDATE epochs e
SET first_tx_sequence_number = c.min_tx_sequence_number
FROM checkpoints c
WHERE e.first_checkpoint_id = c.sequence_number
AND e.epoch = (SELECT MAX(epoch) FROM epochs);
