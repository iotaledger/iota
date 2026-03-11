// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Filter types for event and transaction subscription/querying.
//!
//! These types were originally in `iota-json-rpc-types` but are moved here
//! because they are used internally by `iota-core` (subscription handler,
//! streamer, index) and should not depend on a JSON-RPC presentation crate.

use move_core_types::{identifier::Identifier, language_storage::StructTag};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    base_types::{IotaAddress, ObjectID, ObjectInfo, ObjectType, TransactionDigest},
    effects::{TransactionEffects, TransactionEffectsAPI},
    event::EventEnvelope,
    gas_coin::GasCoin,
    object::Owner,
    transaction::{TransactionData, TransactionDataAPI, TransactionKind},
};

// ---------------------------------------------------------------------------
// Core filter trait
// ---------------------------------------------------------------------------

/// Generic filter trait used by the streaming infrastructure.
pub trait Filter<T> {
    fn matches(&self, item: &T) -> bool;
}

// ---------------------------------------------------------------------------
// EffectsWithInput — bundle of tx data + effects for transaction filtering
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct EffectsWithInput {
    pub effects: TransactionEffects,
    pub input: TransactionData,
}

impl From<EffectsWithInput> for TransactionEffects {
    fn from(e: EffectsWithInput) -> Self {
        e.effects
    }
}

// ---------------------------------------------------------------------------
// Simplified TransactionKind for filter matching
// ---------------------------------------------------------------------------

/// Represents the type of a transaction for filtering purposes.
/// All transactions except `ProgrammableTransaction` are considered system
/// transactions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum SimpleTransactionKind {
    /// Can be used to filter for all types of system transactions.
    SystemTransaction = 0,
    ProgrammableTransaction = 1,
    Genesis = 2,
    ConsensusCommitPrologueV1 = 3,
    AuthenticatorStateUpdateV1 = 4,
    RandomnessStateUpdate = 5,
    EndOfEpochTransaction = 6,
}

impl SimpleTransactionKind {
    pub fn is_system_transaction(&self) -> bool {
        !matches!(self, Self::ProgrammableTransaction)
    }
}

impl From<&TransactionKind> for SimpleTransactionKind {
    fn from(kind: &TransactionKind) -> Self {
        match kind {
            TransactionKind::Genesis(_) => Self::Genesis,
            TransactionKind::ConsensusCommitPrologueV1(_) => Self::ConsensusCommitPrologueV1,
            TransactionKind::AuthenticatorStateUpdateV1(_) => Self::AuthenticatorStateUpdateV1,
            TransactionKind::RandomnessStateUpdate(_) => Self::RandomnessStateUpdate,
            TransactionKind::EndOfEpochTransaction(_) => Self::EndOfEpochTransaction,
            TransactionKind::ProgrammableTransaction(_) => Self::ProgrammableTransaction,
        }
    }
}

// ---------------------------------------------------------------------------
// EventFilter
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum EventFilter {
    /// Query by sender address.
    Sender(IotaAddress),
    /// Return events emitted by the given transaction.
    Transaction(TransactionDigest),
    /// Return events emitted in a specified Package.
    Package(ObjectID),
    /// Return events emitted in a specified Move module.
    MoveModule {
        /// the Move package ID
        package: ObjectID,
        /// the module name
        module: Identifier,
    },
    /// Return events with the given Move event struct name (struct tag).
    MoveEventType(StructTag),
    /// Return events with the given Move module name where the event struct is
    /// defined.
    MoveEventModule {
        /// the Move package ID
        package: ObjectID,
        /// the module name
        module: Identifier,
    },
    MoveEventField {
        path: String,
        value: Value,
    },
    /// Return events emitted in [start_time, end_time) interval
    TimeRange {
        /// left endpoint of time interval, milliseconds since epoch, inclusive
        start_time: u64,
        /// right endpoint of time interval, milliseconds since epoch, exclusive
        end_time: u64,
    },
    All(Vec<EventFilter>),
    Any(Vec<EventFilter>),
    And(Box<EventFilter>, Box<EventFilter>),
    Or(Box<EventFilter>, Box<EventFilter>),
}

impl EventFilter {
    fn try_matches(&self, item: &EventEnvelope) -> bool {
        match self {
            EventFilter::MoveEventType(event_type) => &item.event.type_ == event_type,
            EventFilter::MoveEventField { path, value } => {
                matches!(item.parsed_json.pointer(path), Some(v) if v == value)
            }
            EventFilter::Sender(sender) => &item.event.sender == sender,
            EventFilter::Package(object_id) => &item.event.package_id == object_id,
            EventFilter::MoveModule { package, module } => {
                &item.event.transaction_module == module && &item.event.package_id == package
            }
            EventFilter::All(filters) => filters.iter().all(|f| f.matches(item)),
            EventFilter::Any(filters) => filters.iter().any(|f| f.matches(item)),
            EventFilter::And(f1, f2) => f1.matches(item) && f2.matches(item),
            EventFilter::Or(f1, f2) => f1.matches(item) || f2.matches(item),
            EventFilter::Transaction(digest) => digest == &item.tx_digest,
            EventFilter::TimeRange {
                start_time,
                end_time,
            } => *start_time <= item.timestamp && *end_time > item.timestamp,
            EventFilter::MoveEventModule { package, module } => {
                &item.event.type_.module == module
                    && &ObjectID::from(item.event.type_.address) == package
            }
        }
    }

    pub fn and(self, other_filter: EventFilter) -> Self {
        Self::All(vec![self, other_filter])
    }
    pub fn or(self, other_filter: EventFilter) -> Self {
        Self::Any(vec![self, other_filter])
    }
}

impl Filter<EventEnvelope> for EventFilter {
    fn matches(&self, item: &EventEnvelope) -> bool {
        self.try_matches(item)
    }
}

// ---------------------------------------------------------------------------
// TransactionFilter
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TransactionFilter {
    /// Query by checkpoint.
    Checkpoint(u64),
    /// Query by move function.
    MoveFunction {
        package: ObjectID,
        module: Option<String>,
        function: Option<String>,
    },
    /// Query by input object.
    InputObject(ObjectID),
    /// Query by changed object, including created, mutated and unwrapped
    /// objects.
    ChangedObject(ObjectID),
    /// Query by sender address.
    FromAddress(IotaAddress),
    /// Query by recipient address.
    ToAddress(IotaAddress),
    /// Query by sender and recipient address.
    FromAndToAddress { from: IotaAddress, to: IotaAddress },
    /// Query txs that have a given address as sender or recipient.
    FromOrToAddress { addr: IotaAddress },
    /// Query by transaction kind
    TransactionKind(SimpleTransactionKind),
    /// Query transactions of any given kind in the input.
    TransactionKindIn(Vec<SimpleTransactionKind>),
}

impl TransactionFilter {
    pub fn as_v2(&self) -> TransactionFilterV2 {
        match self {
            TransactionFilter::InputObject(o) => TransactionFilterV2::InputObject(*o),
            TransactionFilter::ChangedObject(o) => TransactionFilterV2::ChangedObject(*o),
            TransactionFilter::FromAddress(a) => TransactionFilterV2::FromAddress(*a),
            TransactionFilter::ToAddress(a) => TransactionFilterV2::ToAddress(*a),
            TransactionFilter::FromAndToAddress { from, to } => {
                TransactionFilterV2::FromAndToAddress {
                    from: *from,
                    to: *to,
                }
            }
            TransactionFilter::FromOrToAddress { addr } => {
                TransactionFilterV2::FromOrToAddress { addr: *addr }
            }
            TransactionFilter::MoveFunction {
                package,
                module,
                function,
            } => TransactionFilterV2::MoveFunction {
                package: *package,
                module: module.clone(),
                function: function.clone(),
            },
            TransactionFilter::TransactionKind(kind) => TransactionFilterV2::TransactionKind(*kind),
            TransactionFilter::TransactionKindIn(kinds) => {
                TransactionFilterV2::TransactionKindIn(kinds.clone())
            }
            TransactionFilter::Checkpoint(checkpoint) => {
                TransactionFilterV2::Checkpoint(*checkpoint)
            }
        }
    }
}

impl Filter<EffectsWithInput> for TransactionFilter {
    // Note: no monitored_scope here — this is a lightweight in-memory match.
    // iota-types intentionally does not depend on iota-metrics. Higher-level
    // callers (subscription_handler, grpc_server) provide their own
    // instrumentation.
    fn matches(&self, item: &EffectsWithInput) -> bool {
        match self {
            TransactionFilter::InputObject(o) => {
                let Ok(input_objects) = item.input.input_objects() else {
                    return false;
                };
                input_objects.iter().any(|object| object.object_id() == *o)
            }
            TransactionFilter::ChangedObject(o) => item
                .effects
                .mutated()
                .iter()
                .chain(item.effects.created().iter())
                .chain(item.effects.unwrapped().iter())
                .any(|(oref, _owner)| &oref.0 == o),
            TransactionFilter::FromAddress(a) => &item.input.sender() == a,
            TransactionFilter::ToAddress(a) => item
                .effects
                .mutated()
                .iter()
                .chain(item.effects.created().iter())
                .chain(item.effects.unwrapped().iter())
                .any(|(_oref, owner)| matches!(owner, Owner::AddressOwner(addr) if addr == a)),
            TransactionFilter::FromAndToAddress { from, to } => {
                Self::FromAddress(*from).matches(item) && Self::ToAddress(*to).matches(item)
            }
            TransactionFilter::FromOrToAddress { addr } => {
                Self::FromAddress(*addr).matches(item) || Self::ToAddress(*addr).matches(item)
            }
            TransactionFilter::MoveFunction {
                package,
                module,
                function,
            } => item.input.move_calls().into_iter().any(|(p, m, f)| {
                p == package
                    && (module.is_none() || matches!(module, Some(m2) if m2.as_str() == m))
                    && (function.is_none() || matches!(function, Some(f2) if f2.as_str() == f))
            }),
            TransactionFilter::TransactionKind(kind) => {
                let actual = SimpleTransactionKind::from(item.input.kind());
                kind == &actual
                    || (*kind == SimpleTransactionKind::SystemTransaction
                        && actual.is_system_transaction())
            }
            TransactionFilter::TransactionKindIn(kinds) => {
                let actual = SimpleTransactionKind::from(item.input.kind());
                kinds.iter().any(|kind| {
                    kind == &actual
                        || (*kind == SimpleTransactionKind::SystemTransaction
                            && actual.is_system_transaction())
                })
            }
            // Checkpoint filter is not supported for subscription, RPC will reject it
            TransactionFilter::Checkpoint(_) => false,
        }
    }
}

// ---------------------------------------------------------------------------
// TransactionFilterV2
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub enum TransactionFilterV2 {
    /// Query by checkpoint.
    Checkpoint(u64),
    /// Query by move function.
    MoveFunction {
        package: ObjectID,
        module: Option<String>,
        function: Option<String>,
    },
    /// Query by input object.
    InputObject(ObjectID),
    /// Query by changed object, including created, mutated and unwrapped
    /// objects.
    ChangedObject(ObjectID),
    /// Query transactions that wrapped or deleted the specified object.
    WrappedOrDeletedObject(ObjectID),
    /// Query by sender address.
    FromAddress(IotaAddress),
    /// Query by recipient address.
    ToAddress(IotaAddress),
    /// Query by sender and recipient address.
    FromAndToAddress { from: IotaAddress, to: IotaAddress },
    /// Query txs that have a given address as sender or recipient.
    FromOrToAddress { addr: IotaAddress },
    /// Query by transaction kind
    TransactionKind(SimpleTransactionKind),
    /// Query transactions of any given kind in the input.
    TransactionKindIn(Vec<SimpleTransactionKind>),
}

impl TransactionFilterV2 {
    pub fn as_v1(&self) -> Option<TransactionFilter> {
        match self {
            TransactionFilterV2::InputObject(o) => Some(TransactionFilter::InputObject(*o)),
            TransactionFilterV2::ChangedObject(o) => Some(TransactionFilter::ChangedObject(*o)),
            TransactionFilterV2::FromAddress(a) => Some(TransactionFilter::FromAddress(*a)),
            TransactionFilterV2::ToAddress(a) => Some(TransactionFilter::ToAddress(*a)),
            TransactionFilterV2::FromAndToAddress { from, to } => {
                Some(TransactionFilter::FromAndToAddress {
                    from: *from,
                    to: *to,
                })
            }
            TransactionFilterV2::FromOrToAddress { addr } => {
                Some(TransactionFilter::FromOrToAddress { addr: *addr })
            }
            TransactionFilterV2::MoveFunction {
                package,
                module,
                function,
            } => Some(TransactionFilter::MoveFunction {
                package: *package,
                module: module.clone(),
                function: function.clone(),
            }),
            TransactionFilterV2::TransactionKind(kind) => {
                Some(TransactionFilter::TransactionKind(*kind))
            }
            TransactionFilterV2::TransactionKindIn(kinds) => {
                Some(TransactionFilter::TransactionKindIn(kinds.clone()))
            }
            TransactionFilterV2::Checkpoint(checkpoint) => {
                Some(TransactionFilter::Checkpoint(*checkpoint))
            }
            // V2-only variants which do not have a V1 equivalent
            TransactionFilterV2::WrappedOrDeletedObject(_) => None,
        }
    }
}

impl Filter<EffectsWithInput> for TransactionFilterV2 {
    fn matches(&self, item: &EffectsWithInput) -> bool {
        match self {
            TransactionFilterV2::Checkpoint(c) => TransactionFilter::Checkpoint(*c).matches(item),
            TransactionFilterV2::MoveFunction {
                package,
                module,
                function,
            } => item.input.move_calls().into_iter().any(|(p, m, f)| {
                p == package
                    && (module.is_none() || matches!(module, Some(m2) if m2.as_str() == m))
                    && (function.is_none() || matches!(function, Some(f2) if f2.as_str() == f))
            }),
            TransactionFilterV2::InputObject(o) => TransactionFilter::InputObject(*o).matches(item),
            TransactionFilterV2::ChangedObject(o) => {
                TransactionFilter::ChangedObject(*o).matches(item)
            }
            TransactionFilterV2::WrappedOrDeletedObject(o) => item
                .effects
                .wrapped()
                .iter()
                .chain(item.effects.deleted().iter())
                .chain(item.effects.unwrapped_then_deleted().iter())
                .any(|oref| &oref.0 == o),
            TransactionFilterV2::FromAddress(a) => TransactionFilter::FromAddress(*a).matches(item),
            TransactionFilterV2::ToAddress(a) => TransactionFilter::ToAddress(*a).matches(item),
            TransactionFilterV2::FromAndToAddress { from, to } => {
                TransactionFilter::FromAddress(*from).matches(item)
                    && TransactionFilter::ToAddress(*to).matches(item)
            }
            TransactionFilterV2::FromOrToAddress { addr } => {
                TransactionFilter::FromAddress(*addr).matches(item)
                    || TransactionFilter::ToAddress(*addr).matches(item)
            }
            TransactionFilterV2::TransactionKind(kind) => {
                TransactionFilter::TransactionKind(*kind).matches(item)
            }
            TransactionFilterV2::TransactionKindIn(kinds) => {
                let actual = SimpleTransactionKind::from(item.input.kind());
                kinds.iter().any(|kind| {
                    kind == &actual
                        || (*kind == SimpleTransactionKind::SystemTransaction
                            && actual.is_system_transaction())
                })
            }
        }
    }
}

// ---------------------------------------------------------------------------
// ObjectDataFilter
// ---------------------------------------------------------------------------

/// Filter for querying objects by various criteria.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ObjectDataFilter {
    MatchAll(Vec<ObjectDataFilter>),
    MatchAny(Vec<ObjectDataFilter>),
    MatchNone(Vec<ObjectDataFilter>),
    /// Query by type for a specified Package.
    Package(ObjectID),
    /// Query by type for a specified Move module.
    MoveModule {
        /// the Move package ID
        package: ObjectID,
        /// the module name
        module: Identifier,
    },
    /// Query by type
    StructType(StructTag),
    AddressOwner(IotaAddress),
    ObjectOwner(ObjectID),
    ObjectId(ObjectID),
    /// allow querying for multiple object ids
    ObjectIds(Vec<ObjectID>),
    Version(u64),
}

impl ObjectDataFilter {
    pub fn gas_coin() -> Self {
        Self::StructType(GasCoin::type_())
    }

    pub fn and(self, other: Self) -> Self {
        Self::MatchAll(vec![self, other])
    }
    pub fn or(self, other: Self) -> Self {
        Self::MatchAny(vec![self, other])
    }
    #[allow(clippy::should_implement_trait)]
    pub fn not(self) -> Self {
        Self::MatchNone(vec![self])
    }

    pub fn matches(&self, object: &ObjectInfo) -> bool {
        match self {
            ObjectDataFilter::MatchAll(filters) => filters.iter().all(|f| f.matches(object)),
            ObjectDataFilter::MatchAny(filters) => filters.iter().any(|f| f.matches(object)),
            ObjectDataFilter::MatchNone(filters) => !filters.iter().any(|f| f.matches(object)),
            ObjectDataFilter::StructType(s) => {
                let obj_tag: StructTag = match &object.type_ {
                    ObjectType::Package => return false,
                    ObjectType::Struct(s) => s.clone().into(),
                };
                // If people do not provide type_params, we will match all type_params
                // e.g. `0x2::coin::Coin` can match `0x2::coin::Coin<0x2::iota::IOTA>`
                if !s.type_params.is_empty() && s.type_params != obj_tag.type_params {
                    false
                } else {
                    obj_tag.address == s.address
                        && obj_tag.module == s.module
                        && obj_tag.name == s.name
                }
            }
            ObjectDataFilter::MoveModule { package, module } => {
                matches!(&object.type_, ObjectType::Struct(s) if &ObjectID::from(s.address()) == package
                        && s.module() == module.as_ident_str())
            }
            ObjectDataFilter::Package(p) => {
                matches!(&object.type_, ObjectType::Struct(s) if &ObjectID::from(s.address()) == p)
            }
            ObjectDataFilter::AddressOwner(a) => {
                matches!(object.owner, Owner::AddressOwner(addr) if &addr == a)
            }
            ObjectDataFilter::ObjectOwner(o) => {
                matches!(object.owner, Owner::ObjectOwner(addr) if addr == IotaAddress::from(*o))
            }
            ObjectDataFilter::ObjectId(id) => &object.object_id == id,
            ObjectDataFilter::ObjectIds(ids) => ids.contains(&object.object_id),
            ObjectDataFilter::Version(v) => object.version.value() == *v,
        }
    }
}
