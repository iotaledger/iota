-- Full BCS-serialized `CheckpointSummary`. The individual columns omit
-- `content_digest` and `version_specific_data`, so they cannot reconstruct it.
ALTER TABLE checkpoints ADD COLUMN checkpoint_summary_bcs BYTEA;
