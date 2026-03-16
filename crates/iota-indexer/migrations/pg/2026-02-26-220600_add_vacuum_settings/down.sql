-- Revert custom autovacuum and index cleanup settings back to default
ALTER TABLE checkpoints RESET (autovacuum_vacuum_scale_factor, autovacuum_vacuum_cost_limit);
ALTER TABLE pruner_cp_watermark RESET (autovacuum_vacuum_scale_factor, autovacuum_vacuum_cost_limit);

ALTER TABLE event_emit_package RESET (autovacuum_vacuum_scale_factor, autovacuum_vacuum_cost_limit);
ALTER TABLE event_emit_module RESET (autovacuum_vacuum_scale_factor, autovacuum_vacuum_cost_limit);
ALTER TABLE event_senders RESET (autovacuum_vacuum_scale_factor, autovacuum_vacuum_cost_limit);
ALTER TABLE event_struct_instantiation RESET (autovacuum_vacuum_scale_factor, autovacuum_vacuum_cost_limit);
ALTER TABLE event_struct_module RESET (autovacuum_vacuum_scale_factor, autovacuum_vacuum_cost_limit);
ALTER TABLE event_struct_name RESET (autovacuum_vacuum_scale_factor, autovacuum_vacuum_cost_limit);
ALTER TABLE event_struct_package RESET (autovacuum_vacuum_scale_factor, autovacuum_vacuum_cost_limit);

ALTER TABLE tx_calls_pkg RESET (autovacuum_vacuum_scale_factor, autovacuum_vacuum_cost_limit);
ALTER TABLE tx_calls_mod RESET (autovacuum_vacuum_scale_factor, autovacuum_vacuum_cost_limit);
ALTER TABLE tx_calls_fun RESET (autovacuum_vacuum_scale_factor, autovacuum_vacuum_cost_limit);
ALTER TABLE tx_changed_objects RESET (autovacuum_vacuum_scale_factor, autovacuum_vacuum_cost_limit);
ALTER TABLE tx_digests RESET (autovacuum_vacuum_scale_factor, autovacuum_vacuum_cost_limit);
ALTER TABLE tx_input_objects RESET (autovacuum_vacuum_scale_factor, autovacuum_vacuum_cost_limit);
ALTER TABLE tx_kinds RESET (autovacuum_vacuum_scale_factor, autovacuum_vacuum_cost_limit);
ALTER TABLE tx_recipients RESET (autovacuum_vacuum_scale_factor, autovacuum_vacuum_cost_limit);
ALTER TABLE tx_senders RESET (autovacuum_vacuum_scale_factor, autovacuum_vacuum_cost_limit);
ALTER TABLE tx_wrapped_or_deleted_objects RESET (autovacuum_vacuum_scale_factor, autovacuum_vacuum_cost_limit);
ALTER TABLE tx_global_order RESET (autovacuum_vacuum_scale_factor, autovacuum_vacuum_cost_limit);

ALTER TABLE optimistic_transactions RESET (autovacuum_vacuum_scale_factor, autovacuum_vacuum_cost_limit);
