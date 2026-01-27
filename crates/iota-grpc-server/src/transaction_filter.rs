// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_json_rpc_types::{
    Filter, IotaTransactionBlockEffects, IotaTransactionBlockEffectsAPI,
    IotaTransactionBlockEvents, IotaTransactionKind, OwnedObjectRef,
};
use iota_metrics::monitored_scope;
use iota_types::{
    base_types::{IotaAddress, ObjectID},
    object::Owner,
    transaction::{TransactionData, TransactionDataAPI},
};
use serde::{Deserialize, Serialize};

use crate::event_filter::GrpcEventFilter;

#[derive(Clone)]
pub struct TransactionDataWithEffectsAndEvents {
    pub tx_data: TransactionData,
    pub effects: IotaTransactionBlockEffects,
    pub events: IotaTransactionBlockEvents,
}

impl From<TransactionDataWithEffectsAndEvents> for IotaTransactionBlockEffects {
    fn from(e: TransactionDataWithEffectsAndEvents) -> Self {
        e.effects
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum GrpcTransactionFilter {
    // Logical AND of several filters.
    All(Vec<GrpcTransactionFilter>),
    // Logical OR of several filters.
    Any(Vec<GrpcTransactionFilter>),
    // Logical NOT of a filter.
    Not(Box<GrpcTransactionFilter>),

    /// Filter transactions of any given kind in the input.
    TransactionKind(Vec<IotaTransactionKind>),

    /// Filter by sender address.
    Sender(IotaAddress),
    /// Filter by recipient address. The recipient is determined by
    /// checking the owners of mutated and unwrapped objects.
    Receiver(IotaAddress),

    /// Filter by input object.
    InputObject(ObjectID),
    /// Filter by changed object, including created, mutated and unwrapped
    /// objects.
    ChangedObject(ObjectID),
    /// Filter transactions that wrapped or deleted the specified object.
    /// Includes transactions that either created and immediately wrapped
    /// the object or unwrapped and immediately deleted it.
    /// TODO: @infra: do we need that now that we have the AffectedObject
    /// filter?
    WrappedOrDeletedObject(ObjectID),
    /// Filter for transactions that touch this object.
    AffectedObject(ObjectID),

    /// Filter by move package, module (optional) and function (optional).
    MoveCall {
        /// the Move package ID
        package: ObjectID,
        /// the module name
        module: Option<String>,
        /// the function name
        function: Option<String>,
    },

    /// Filter transactions that contain events matching the given event filter.
    Events(GrpcEventFilter),
}

impl Filter<TransactionDataWithEffectsAndEvents> for GrpcTransactionFilter {
    fn matches(&self, item: &TransactionDataWithEffectsAndEvents) -> bool {
        let _scope = monitored_scope("GrpcTransactionFilter::matches");
        match self {
            GrpcTransactionFilter::All(filters) => filters.iter().all(|f| f.matches(item)),
            GrpcTransactionFilter::Any(filters) => filters.iter().any(|f| f.matches(item)),
            GrpcTransactionFilter::Not(filter) => !filter.matches(item),

            GrpcTransactionFilter::TransactionKind(kinds) => kinds
                .iter()
                .any(|kind| kind == &IotaTransactionKind::from(item.tx_data.kind())),

            GrpcTransactionFilter::Sender(a) => &item.tx_data.sender() == a,
            GrpcTransactionFilter::Receiver(a) => {
                let mutated: &[OwnedObjectRef] = item.effects.mutated();
                mutated.iter().chain(item.effects.unwrapped().iter()).any(|oref: &OwnedObjectRef| {
                    matches!(oref.owner, Owner::AddressOwner(owner) if owner == *a)
                })
            }

            GrpcTransactionFilter::InputObject(o) => {
                let Ok(input_objects) = item.tx_data.input_objects() else {
                    return false;
                };
                input_objects.iter().any(|object| object.object_id() == *o)
            }
            GrpcTransactionFilter::ChangedObject(o) => item
                .effects
                .mutated()
                .iter()
                .any(|oref: &OwnedObjectRef| &oref.reference.object_id == o),
            GrpcTransactionFilter::WrappedOrDeletedObject(o) => item
                .effects
                .wrapped()
                .iter()
                .chain(item.effects.deleted().iter())
                .chain(item.effects.unwrapped_then_deleted().iter())
                .any(|oref| &oref.object_id == o),
            GrpcTransactionFilter::AffectedObject(o) => item
                .effects
                .created()
                .iter()
                .chain(item.effects.mutated().iter())
                .chain(item.effects.unwrapped().iter())
                .map(|oref: &OwnedObjectRef| &oref.reference)
                .chain(item.effects.shared_objects().iter())
                .chain(item.effects.deleted().iter())
                .chain(item.effects.unwrapped_then_deleted().iter())
                .chain(item.effects.wrapped().iter())
                .any(|oref| &oref.object_id == o),

            GrpcTransactionFilter::MoveCall {
                package,
                module,
                function,
            } => item.tx_data.move_calls().into_iter().any(|(p, m, f)| {
                p == package
                    && (module.is_none() || matches!(module,  Some(m2) if m2 == &m.to_string()))
                    && (function.is_none() || matches!(function, Some(f2) if f2 == &f.to_string()))
            }),

            GrpcTransactionFilter::Events(event_filter) => item
                .events
                .data
                .iter()
                .any(|event| event_filter.matches(event)),
        }
    }
}
