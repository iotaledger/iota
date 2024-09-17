// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#![allow(clippy::all)]

#[cfg(feature = "mysql-feature")]
#[cfg(not(feature = "postgres-feature"))]
mod mysql;

#[cfg(feature = "postgres-feature")]
mod pg;

#[cfg(feature = "postgres-feature")]
mod inner {
    pub use crate::schema::pg::{
        chain_identifier, checkpoints, display, epochs, event_emit_module, event_emit_package,
        event_senders, event_struct_instantiation, event_struct_module, event_struct_name,
        event_struct_package, events, feature_flags, objects, objects_history, objects_snapshot,
        objects_version, packages, protocol_configs, pruner_cp_watermark, transactions,
        tx_calls_fun, tx_calls_mod, tx_calls_pkg, tx_changed_objects, tx_digests, tx_input_objects,
        tx_kinds, tx_recipients, tx_senders,
    };
}

#[cfg(feature = "mysql-feature")]
#[cfg(not(feature = "postgres-feature"))]
mod inner {
    pub use crate::schema::mysql::{
        chain_identifier, checkpoints, display, epochs, event_emit_module, event_emit_package,
        event_senders, event_struct_instantiation, event_struct_module, event_struct_name,
        event_struct_package, events, feature_flags, objects, objects_history, objects_snapshot,
        objects_version, packages, protocol_configs, pruner_cp_watermark, transactions,
        tx_calls_fun, tx_calls_mod, tx_calls_pkg, tx_changed_objects, tx_digests, tx_input_objects,
        tx_kinds, tx_recipients, tx_senders,
    };
}

pub use inner::{
    chain_identifier, checkpoints, display, epochs, event_emit_module, event_emit_package,
    event_senders, event_struct_instantiation, event_struct_module, event_struct_name,
    event_struct_package, events, feature_flags, objects, objects_history, objects_snapshot,
    objects_version, packages, protocol_configs, pruner_cp_watermark, transactions, tx_calls_fun,
    tx_calls_mod, tx_calls_pkg, tx_changed_objects, tx_digests, tx_input_objects, tx_kinds,
    tx_recipients, tx_senders,
};

// Postgres only tables
#[cfg(feature = "postgres-feature")]
pub use crate::schema::pg::events_partition_0;
#[cfg(feature = "postgres-feature")]
pub use crate::schema::pg::objects_history_partition_0;
#[cfg(feature = "postgres-feature")]
pub use crate::schema::pg::transactions_partition_0;
