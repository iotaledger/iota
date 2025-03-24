-- A materialized view that the total number of unique addresses that have delegated stake in the current epoch.
-- Includes both staked and timelocked staked IOTA.
CREATE MATERIALIZED VIEW participation_metrics AS
SELECT
    COUNT(DISTINCT owner_id) AS total_addresses
FROM
    objects
WHERE
        object_type IN ('0x0000000000000000000000000000000000000000000000000000000000000003::staking_pool::StakedIota', '0x0000000000000000000000000000000000000000000000000000000000000003::timelocked_staking::TimelockedStakedIota')
  AND owner_id IS NOT NULL;
