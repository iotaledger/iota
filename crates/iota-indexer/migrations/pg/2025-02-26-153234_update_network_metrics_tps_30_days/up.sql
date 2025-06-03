CREATE OR REPLACE VIEW network_metrics AS
SELECT  (SELECT recent_tps from real_time_tps)                                                          AS current_tps,
        COALESCE((SELECT peak_tps_30d FROM epoch_peak_tps ORDER BY epoch DESC LIMIT 1), 0)             AS tps_30_days,
        (SELECT reltuples AS estimate FROM pg_class WHERE relname = 'addresses')::BIGINT                AS total_addresses,
        (SELECT reltuples AS estimate FROM pg_class WHERE relname = 'objects')::BIGINT                  AS total_objects,
        (SELECT reltuples AS estimate FROM pg_class WHERE relname = 'packages')::BIGINT                 AS total_packages,
        (SELECT MAX(epoch) FROM epochs)                                                                 AS current_epoch,
        (SELECT MAX(sequence_number) FROM checkpoints)                                                  AS current_checkpoint;
