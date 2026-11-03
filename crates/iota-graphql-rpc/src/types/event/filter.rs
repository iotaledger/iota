// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use async_graphql::*;
use iota_indexer::pruning::{
    CommitterTables, oldest_available_checkpoint::OldestAvailableCheckpoint,
};

use crate::{
    data::Db,
    types::{
        digest::Digest,
        event::lookups::lookup_table,
        iota_address::IotaAddress,
        type_filter::{ModuleFilter, TypeFilter},
    },
};

/// Represents optional available filters for events.
#[derive(InputObject, Clone, Default)]
pub(crate) struct EventFilter {
    /// Filter down to events from transactions sent by this address.
    pub sender: Option<IotaAddress>,
    /// Filter down to the events from this transaction (given by its
    /// transaction digest).
    pub transaction_digest: Option<Digest>,
    // Enhancement (post-MVP)
    // after_checkpoint
    // at_checkpoint
    // before_checkpoint
    /// Events emitted by a particular module. An event is emitted by a
    /// particular module if some function in the module is called by a
    /// PTB and emits an event.
    ///
    /// Modules can be filtered by their package, or package::module.
    /// We currently do not support filtering by emitting module and event type
    /// at the same time so if both are provided in one filter, the query will
    /// error.
    pub emitting_module: Option<ModuleFilter>,

    /// This field is used to specify the type of event emitted.
    ///
    /// Events can be filtered by their type's package, package::module,
    /// or their fully qualified type name.
    ///
    /// Generic types can be queried by either the generic type name, e.g.
    /// `0x2::coin::Coin`, or by the full type name, such as
    /// `0x2::coin::Coin<0x2::iota::IOTA>`.
    pub event_type: Option<TypeFilter>,
    // Enhancement (post-MVP)
    // pub start_time
    // pub end_time

    // Enhancement (post-MVP)
    // pub any
    // pub all
    // pub not
}

impl EventFilter {
    /// Returns the transaction digest when `transaction_digest` is the only
    /// filter set.
    pub(crate) fn only_transaction_digest(&self) -> Option<Digest> {
        if self.sender.is_some() || self.emitting_module.is_some() || self.event_type.is_some() {
            return None;
        }
        self.transaction_digest
    }

    /// The oldest checkpoint a page of events with this filter is available
    /// from: the value reported on `DATA_PRUNED`.
    pub(crate) fn oldest_available_checkpoint(&self, db: &Db) -> OldestAvailableCheckpoint {
        if self.only_transaction_digest().is_some() {
            return OldestAvailableCheckpoint::events_by_tx_digest(&db.inner);
        }

        // The general query reads `events`, the lookup table of the filter, and
        // `tx_global_order` for the transaction digest.
        let mut tables = vec![CommitterTables::Events];
        tables.extend(lookup_table(self));
        if self.transaction_digest.is_some() {
            tables.push(CommitterTables::TxGlobalOrder);
        }
        OldestAvailableCheckpoint::for_tables(&db.inner, &tables, false)
    }
}
