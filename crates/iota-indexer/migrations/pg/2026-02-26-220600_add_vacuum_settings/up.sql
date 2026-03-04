-- Apply more frequent autovacuum and index cleanup settings to prunable tables
ALTER TABLE checkpoints SET (vacuum_index_cleanup=on, autovacuum_vacuum_scale_factor=0.01, autovacuum_enabled=on);
ALTER TABLE pruner_cp_watermark SET (vacuum_index_cleanup=on, autovacuum_vacuum_scale_factor=0.01, autovacuum_enabled=on);

ALTER TABLE event_emit_package SET (vacuum_index_cleanup=on, autovacuum_vacuum_scale_factor=0.01, autovacuum_enabled=on);
ALTER TABLE event_emit_module SET (vacuum_index_cleanup=on, autovacuum_vacuum_scale_factor=0.01, autovacuum_enabled=on);
ALTER TABLE event_senders SET (vacuum_index_cleanup=on, autovacuum_vacuum_scale_factor=0.01, autovacuum_enabled=on);
ALTER TABLE event_struct_instantiation SET (vacuum_index_cleanup=on, autovacuum_vacuum_scale_factor=0.01, autovacuum_enabled=on);
ALTER TABLE event_struct_module SET (vacuum_index_cleanup=on, autovacuum_vacuum_scale_factor=0.01, autovacuum_enabled=on);
ALTER TABLE event_struct_name SET (vacuum_index_cleanup=on, autovacuum_vacuum_scale_factor=0.01, autovacuum_enabled=on);
ALTER TABLE event_struct_package SET (vacuum_index_cleanup=on, autovacuum_vacuum_scale_factor=0.01, autovacuum_enabled=on);

ALTER TABLE tx_calls_pkg SET (vacuum_index_cleanup=on, autovacuum_vacuum_scale_factor=0.01, autovacuum_enabled=on);
ALTER TABLE tx_calls_mod SET (vacuum_index_cleanup=on, autovacuum_vacuum_scale_factor=0.01, autovacuum_enabled=on);
ALTER TABLE tx_calls_fun SET (vacuum_index_cleanup=on, autovacuum_vacuum_scale_factor=0.01, autovacuum_enabled=on);
ALTER TABLE tx_changed_objects SET (vacuum_index_cleanup=on, autovacuum_vacuum_scale_factor=0.01, autovacuum_enabled=on);
ALTER TABLE tx_digests SET (vacuum_index_cleanup=on, autovacuum_vacuum_scale_factor=0.01, autovacuum_enabled=on);
ALTER TABLE tx_input_objects SET (vacuum_index_cleanup=on, autovacuum_vacuum_scale_factor=0.01, autovacuum_enabled=on);
ALTER TABLE tx_kinds SET (vacuum_index_cleanup=on, autovacuum_vacuum_scale_factor=0.01, autovacuum_enabled=on);
ALTER TABLE tx_recipients SET (vacuum_index_cleanup=on, autovacuum_vacuum_scale_factor=0.01, autovacuum_enabled=on);
ALTER TABLE tx_senders SET (vacuum_index_cleanup=on, autovacuum_vacuum_scale_factor=0.01, autovacuum_enabled=on);
ALTER TABLE tx_wrapped_or_deleted_objects SET (vacuum_index_cleanup=on, autovacuum_vacuum_scale_factor=0.01, autovacuum_enabled=on);
ALTER TABLE tx_global_order SET (vacuum_index_cleanup=on, autovacuum_vacuum_scale_factor=0.01, autovacuum_enabled=on);

ALTER TABLE optimistic_transactions SET (vacuum_index_cleanup=on, autovacuum_vacuum_scale_factor=0.01, autovacuum_enabled=on);
