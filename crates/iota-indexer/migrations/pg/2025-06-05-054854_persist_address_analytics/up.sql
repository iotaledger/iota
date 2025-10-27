-- This procedure creates a temporary table to hold the activity data
-- for both senders and recipients, joining once with the transactions table,
-- and ensuring partition pruning during query planning.
--
-- It then updates the `active_addresses` table with metrics for senders only, and
-- `addresses` table with metrics for both senders and recipients.
CREATE OR REPLACE PROCEDURE persist_address_analytics(start_seq bigint, end_seq bigint)
LANGUAGE plpgsql
AS $$
BEGIN
  CREATE TEMP TABLE temp_joined_activity ON COMMIT DROP AS
  WITH all_roles AS (
    SELECT sender AS address, tx_sequence_number, TRUE as is_sender
    FROM tx_senders
    WHERE tx_sequence_number >= start_seq AND tx_sequence_number < end_seq

    UNION ALL

    SELECT recipient AS address, tx_sequence_number, FALSE as is_sender
    FROM tx_recipients
    WHERE tx_sequence_number >= start_seq AND tx_sequence_number < end_seq
  )
  SELECT
    ar.address,
    ar.tx_sequence_number,
    ar.is_sender,
    t.timestamp_ms
  FROM all_roles ar
  JOIN transactions t
    ON ar.tx_sequence_number = t.tx_sequence_number
    -- Ensure partition pruning
    WHERE t.tx_sequence_number >= start_seq AND t.tx_sequence_number < end_seq;

  INSERT INTO active_addresses (
    address,
    first_appearance_tx,
    first_appearance_time,
    last_appearance_tx,
    last_appearance_time
  )
  SELECT
    address,
    MIN(tx_sequence_number),
    MIN(timestamp_ms),
    MAX(tx_sequence_number),
    MAX(timestamp_ms)
  FROM temp_joined_activity
  WHERE is_sender
  GROUP BY address
  ON CONFLICT (address) DO UPDATE
  SET
    last_appearance_tx = GREATEST(EXCLUDED.last_appearance_tx, active_addresses.last_appearance_tx),
    last_appearance_time = GREATEST(EXCLUDED.last_appearance_time, active_addresses.last_appearance_time);

  INSERT INTO addresses (
    address,
    first_appearance_tx,
    first_appearance_time,
    last_appearance_tx,
    last_appearance_time
  )
  SELECT
    address,
    MIN(tx_sequence_number),
    MIN(timestamp_ms),
    MAX(tx_sequence_number),
    MAX(timestamp_ms)
  FROM temp_joined_activity
  GROUP BY address
  ON CONFLICT (address) DO UPDATE
  SET
    last_appearance_tx = GREATEST(EXCLUDED.last_appearance_tx, addresses.last_appearance_tx),
    last_appearance_time = GREATEST(EXCLUDED.last_appearance_time, addresses.last_appearance_time);
END;
$$;
