// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_json_rpc_types::{Filter, IotaEvent};
use iota_metrics::monitored_scope;
use iota_types::{
    base_types::{IotaAddress, ObjectID},
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

    /// Filter by sender address.
    Sender(IotaAddress),

    /// Return events emitted in a specified Move package + module (optional).
    /// If the event is defined in PackageA::ModuleA but emitted in a tx with
    /// PackageB::ModuleB, filtering `MovePackageAndModule` by PackageB::ModuleB
    /// returns the event. Filtering `MoveEventPackageAndModule` by
    /// PackageA::ModuleA returns the event too.
    MovePackageAndModule {
        /// the Move package ID
        package: ObjectID,
        /// the module name (optional)
        module: Option<Identifier>,
    },
    /// Return events with the given Move package + module (optional) where the
    /// event struct is defined. If the event is defined in
    /// PackageA::ModuleA but emitted in a tx with PackageB::ModuleB, filtering
    /// `MoveEventPackageAndModule` by PackageA::ModuleA returns the
    /// event. Filtering `MovePackageAndModule` by PackageB::ModuleB returns the
    /// event too.
    MoveEventPackageAndModule {
        /// the Move package ID
        package: ObjectID,
        /// the module name (optional)
        module: Option<Identifier>,
    },
    /// Return events with the given Move event struct name (struct tag).
    /// For example, if the event is defined in `0xabcd::MyModule`, and named
    /// `Foo`, then the struct tag is `0xabcd::MyModule::Foo`.
    MoveEventType(StructTag),
    /// Return events whose JSON representation contains the given field path
    /// with the specified value (optional). The path should be a JSON pointer
    /// as defined in RFC 6901.
    MoveEventField {
        path: String,
        value: Option<Value>,
    },
}

impl GrpcEventFilter {
    fn try_matches(&self, item: &IotaEvent) -> IotaResult<bool> {
        Ok(match self {
            GrpcEventFilter::All(filters) => filters.iter().all(|f| f.matches(item)),
            GrpcEventFilter::Any(filters) => filters.iter().any(|f| f.matches(item)),
            GrpcEventFilter::Not(filter) => !filter.matches(item),

            GrpcEventFilter::Sender(sender) => &item.sender == sender,

            GrpcEventFilter::MovePackageAndModule { package, module } => {
                &item.package_id == package
                    && (module.is_none()
                        || matches!(module,  Some(m2) if m2 == &item.transaction_module))
            }
            GrpcEventFilter::MoveEventPackageAndModule { package, module } => {
                &ObjectID::from(item.type_.address) == package
                    && (module.is_none() || matches!(module,  Some(m2) if m2 == &item.type_.module))
            }
            GrpcEventFilter::MoveEventType(event_type) => &item.type_ == event_type,
            GrpcEventFilter::MoveEventField { path, value } => {
                let json_ptr_value = item.parsed_json.pointer(path);
                if value.is_none() {
                    // If no value is specified, just check for the existence of the field.
                    json_ptr_value.is_some()
                } else {
                    matches!(json_ptr_value, Some(v) if v == value.as_ref().unwrap())
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
