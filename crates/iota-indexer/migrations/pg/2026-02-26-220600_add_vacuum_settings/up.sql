-- Apply more frequent autovacuum and index cleanup settings to prunable tables
ALTER TABLE checkpoints SET (autovacuum_vacuum_scale_factor=0.01, autovacuum_vacuum_cost_limit=500);
ALTER TABLE pruner_cp_watermark SET (autovacuum_vacuum_scale_factor=0.01, autovacuum_vacuum_cost_limit=500);

ALTER TABLE event_emit_package SET (autovacuum_vacuum_scale_factor=0.01, autovacuum_vacuum_cost_limit=500);
ALTER TABLE event_emit_module SET (autovacuum_vacuum_scale_factor=0.01, autovacuum_vacuum_cost_limit=500);
ALTER TABLE event_senders SET (autovacuum_vacuum_scale_factor=0.01, autovacuum_vacuum_cost_limit=500);
ALTER TABLE event_struct_instantiation SET (autovacuum_vacuum_scale_factor=0.01, autovacuum_vacuum_cost_limit=500);
ALTER TABLE event_struct_module SET (autovacuum_vacuum_scale_factor=0.01, autovacuum_vacuum_cost_limit=500);
ALTER TABLE event_struct_name SET (autovacuum_vacuum_scale_factor=0.01, autovacuum_vacuum_cost_limit=500);
ALTER TABLE event_struct_package SET (autovacuum_vacuum_scale_factor=0.01, autovacuum_vacuum_cost_limit=500);

ALTER TABLE tx_calls_pkg SET (autovacuum_vacuum_scale_factor=0.01, autovacuum_vacuum_cost_limit=500);
ALTER TABLE tx_calls_mod SET (autovacuum_vacuum_scale_factor=0.01, autovacuum_vacuum_cost_limit=500);
ALTER TABLE tx_calls_fun SET (autovacuum_vacuum_scale_factor=0.01, autovacuum_vacuum_cost_limit=500);
ALTER TABLE tx_changed_objects SET (autovacuum_vacuum_scale_factor=0.01, autovacuum_vacuum_cost_limit=500);
ALTER TABLE tx_digests SET (autovacuum_vacuum_scale_factor=0.01, autovacuum_vacuum_cost_limit=500);
ALTER TABLE tx_input_objects SET (autovacuum_vacuum_scale_factor=0.01, autovacuum_vacuum_cost_limit=500);
ALTER TABLE tx_kinds SET (autovacuum_vacuum_scale_factor=0.01, autovacuum_vacuum_cost_limit=500);
ALTER TABLE tx_recipients SET (autovacuum_vacuum_scale_factor=0.01, autovacuum_vacuum_cost_limit=500);
ALTER TABLE tx_senders SET (autovacuum_vacuum_scale_factor=0.01, autovacuum_vacuum_cost_limit=500);
ALTER TABLE tx_wrapped_or_deleted_objects SET (autovacuum_vacuum_scale_factor=0.01, autovacuum_vacuum_cost_limit=500);
ALTER TABLE tx_global_order SET (autovacuum_vacuum_scale_factor=0.01, autovacuum_vacuum_cost_limit=500);

ALTER TABLE optimistic_transactions SET (autovacuum_vacuum_scale_factor=0.01, autovacuum_vacuum_cost_limit=500);
