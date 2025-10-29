// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_json_rpc_types::{Filter, IotaEvent};
use iota_metrics::monitored_scope;
use iota_types::{
    base_types::{IotaAddress, ObjectID, TransactionDigest},
    error::IotaResult,
};
use move_core_types::{identifier::Identifier, language_storage::StructTag};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum GrpcEventFilter {
    // Logical AND of several filters.
    All(Vec<GrpcEventFilter>),
    // Logical OR of several filters.
    Any(Vec<GrpcEventFilter>),
    // Logical NOT of a filter.
    Not(Box<GrpcEventFilter>),

    /// Return events emitted by the given transaction.
    /// TODO: do we need that filter for streaming?
    Transaction(
        /// digest of the transaction, as base-64 encoded string
        TransactionDigest,
    ),

    /// Query by sender address.
    /// TODO: Is the same as the transaction's sender address. Do we need both?
    Sender(IotaAddress),

    /// Return events emitted in a specified Move package.
    MovePackage(ObjectID),

    /// Return events emitted in a specified Move module.
    /// If the event is defined in Module A but emitted in a tx with Module B,
    /// query `MoveModule` by module B returns the event.
    /// Query `MoveEventModule` by module A returns the event too.
    MoveModule {
        /// the Move package ID
        package: ObjectID,
        /// the module name
        module: Identifier,
    },
    /// Return events with the given Move module name where the event struct is
    /// defined. If the event is defined in Module A but emitted in a tx
    /// with Module B, query `MoveEventModule` by module A returns the
    /// event. Query `MoveModule` by module B returns the event too.
    MoveEventModule {
        /// the Move package ID
        package: ObjectID,
        /// the module name
        module: Identifier,
    },
    /// Return events with the given Move event struct name (struct tag).
    /// For example, if the event is defined in `0xabcd::MyModule`, and named
    /// `Foo`, then the struct tag is `0xabcd::MyModule::Foo`.
    MoveEventType(StructTag),
    /// Return events whose JSON representation contains the given field path
    /// with the specified value. The path should be a JSON pointer as
    /// defined in RFC 6901.
    MoveEventField {
        path: String,
        value: Value,
    },

    /// Return events emitted in [start_time, end_time] interval
    TimeRange {
        /// left endpoint of time interval, milliseconds since epoch, inclusive
        start_time: u64,
        /// right endpoint of time interval, milliseconds since epoch, exclusive
        end_time: u64,
    },
}

impl GrpcEventFilter {
    fn try_matches(&self, item: &IotaEvent) -> IotaResult<bool> {
        Ok(match self {
            GrpcEventFilter::All(filters) => filters.iter().all(|f| f.matches(item)),
            GrpcEventFilter::Any(filters) => filters.iter().any(|f| f.matches(item)),
            GrpcEventFilter::Not(filter) => !filter.matches(item),

            GrpcEventFilter::Transaction(digest) => &item.id.tx_digest == digest,

            GrpcEventFilter::Sender(sender) => &item.sender == sender,

            GrpcEventFilter::MovePackage(object_id) => &item.package_id == object_id,

            GrpcEventFilter::MoveModule { package, module } => {
                &item.transaction_module == module && &item.package_id == package
            }
            GrpcEventFilter::MoveEventType(event_type) => &item.type_ == event_type,
            GrpcEventFilter::MoveEventModule { package, module } => {
                &item.type_.module == module && &ObjectID::from(item.type_.address) == package
            }
            GrpcEventFilter::MoveEventField { path, value } => {
                matches!(item.parsed_json.pointer(path), Some(v) if v == value)
            }

            GrpcEventFilter::TimeRange {
                start_time,
                end_time,
            } => {
                if let Some(timestamp) = &item.timestamp_ms {
                    start_time <= timestamp && end_time > timestamp
                } else {
                    false
                }
            }
        })
    }

    pub fn and(self, other_filter: GrpcEventFilter) -> Self {
        Self::All(vec![self, other_filter])
    }
    pub fn or(self, other_filter: GrpcEventFilter) -> Self {
        Self::Any(vec![self, other_filter])
    }
}

impl Filter<IotaEvent> for GrpcEventFilter {
    fn matches(&self, item: &IotaEvent) -> bool {
        let _scope = monitored_scope("GrpcEventFilter::matches");
        self.try_matches(item).unwrap_or_default()
    }
}
