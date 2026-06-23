-- `CheckpointSummary` fields the other columns can't reproduce, needed to
-- reconstruct the summary and recompute its canonical digest.
ALTER TABLE checkpoints ADD COLUMN content_digest BYTEA;
ALTER TABLE checkpoints ADD COLUMN version_specific_data BYTEA;
