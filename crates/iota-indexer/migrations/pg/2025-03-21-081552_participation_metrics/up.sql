CREATE MATERIALIZED VIEW participation_metrics AS
SELECT
    COUNT(DISTINCT owner_id) AS total_addresses,
FROM
    objects
WHERE
        object_type IN ('StakedIota', 'TimelockedStakedIota')
  AND owner_id IS NOT NULL;
