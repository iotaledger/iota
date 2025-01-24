// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#![allow(clippy::all)]

mod pg;

mod inner {
    pub use crate::schema::pg::{
        active_addresses, address_metrics, addresses, chain_identifier, checkpoints, display,
        epoch_peak_tps, epochs, event_emit_module, event_emit_package, event_senders,
        event_struct_instantiation, event_struct_module, event_struct_name, event_struct_package,
        events, feature_flags, move_call_metrics, move_calls, objects, objects_history,
        objects_snapshot, objects_version, packages, protocol_configs, pruner_cp_watermark,
        transactions, tx_calls_fun, tx_calls_mod, tx_calls_pkg, tx_changed_objects,
        tx_count_metrics, tx_digests, tx_input_objects, tx_kinds, tx_recipients, tx_senders,
    };
}

pub use inner::{
    active_addresses, address_metrics, addresses, chain_identifier, checkpoints, display,
    epoch_peak_tps, epochs, event_emit_module, event_emit_package, event_senders,
    event_struct_instantiation, event_struct_module, event_struct_name, event_struct_package,
    events, feature_flags, move_call_metrics, move_calls, objects, objects_history,
    objects_snapshot, objects_version, packages, protocol_configs, pruner_cp_watermark,
    transactions, tx_calls_fun, tx_calls_mod, tx_calls_pkg, tx_changed_objects, tx_count_metrics,
    tx_digests, tx_input_objects, tx_kinds, tx_recipients, tx_senders,
};

pub use crate::schema::pg::{
    events_partition_0, objects_history_partition_0, transactions_partition_0,
};
