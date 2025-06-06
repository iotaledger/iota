-- This SQL script creates a procedure to calculate transaction count metrics
-- based on checkpoints and transactions.
--
-- It uses a transactions to first determine the explicit bounds of transaction
-- sequence numbers from the checkpoints. Then it uses those explicit bounds
-- to trigger static partition pruning while joining the transactions table.
CREATE OR REPLACE PROCEDURE calculate_tx_count_metrics(
    start_checkpoint_number BIGINT,
    end_checkpoint_number BIGINT
)
LANGUAGE plpgsql
AS $$
DECLARE
    min_seq BIGINT;
    max_seq BIGINT;
    range_start BIGINT;
    range_end BIGINT;
BEGIN
    -- Determine tx_sequence_number bounds from checkpoint range
    SELECT MIN(min_tx_sequence_number), MAX(max_tx_sequence_number)
    INTO min_seq, max_seq
    FROM checkpoints
    WHERE sequence_number >= start_checkpoint_number
      AND sequence_number < end_checkpoint_number;

    INSERT INTO tx_count_metrics (
        checkpoint_sequence_number,
        epoch,
        timestamp_ms,
        total_transaction_blocks,
        total_successful_transaction_blocks,
        total_successful_transactions
    )
    SELECT
        c.sequence_number,
        c.epoch,
        MAX(t.timestamp_ms),
        COUNT(*) AS total_transaction_blocks,
        SUM(CASE WHEN t.success_command_count > 0 THEN 1 ELSE 0 END),
        SUM(t.success_command_count)
    FROM checkpoints c
    JOIN transactions t
      ON t.tx_sequence_number BETWEEN c.min_tx_sequence_number AND c.max_tx_sequence_number
    WHERE c.sequence_number >= start_checkpoint_number
      AND c.sequence_number < end_checkpoint_number
      -- Ensure partition pruning by using the explicit bounds
      AND t.tx_sequence_number BETWEEN min_seq AND max_seq
    GROUP BY c.sequence_number, c.epoch
    ON CONFLICT (checkpoint_sequence_number) DO UPDATE
        SET timestamp_ms = EXCLUDED.timestamp_ms,
            total_transaction_blocks = EXCLUDED.total_transaction_blocks,
            total_successful_transaction_blocks = EXCLUDED.total_successful_transaction_blocks,
            total_successful_transactions = EXCLUDED.total_successful_transactions;


END $$;
