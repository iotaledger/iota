// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_json_rpc_types::Filter;
use iota_types::{
    base_types::{IotaAddress, ObjectID},
    effects::{TransactionEffects, TransactionEffectsAPI},
    transaction::{TransactionData, TransactionDataAPI, TransactionKind},
};
use serde::{Deserialize, Serialize};

/// Effects with input for gRPC streaming
/// Contains both the transaction effects and the input transaction data for
/// filtering
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EffectsWithInput {
    pub effects: TransactionEffects,
    pub input: TransactionData,
}

impl From<EffectsWithInput> for TransactionEffects {
    fn from(e: EffectsWithInput) -> Self {
        e.effects
    }
}

/// Transaction filter for gRPC streaming
#[derive(Clone, Debug)]
pub enum TransactionFilter {
    /// Filter by move function
    MoveFunction {
        package: ObjectID,
        module: Option<String>,
        function: Option<String>,
    },
    /// Filter by input object
    InputObject(ObjectID),
    /// Filter by changed object (created, mutated, unwrapped)
    ChangedObject(ObjectID),
    /// Filter by sender address
    FromAddress(IotaAddress),
    /// Filter by recipient address
    ToAddress(IotaAddress),
    /// Filter by sender and recipient address
    FromAndToAddress { from: IotaAddress, to: IotaAddress },
    /// Filter by sender or recipient address
    FromOrToAddress { addr: IotaAddress },
    /// Filter by transaction kind
    TransactionKind(IotaTransactionKind),
    /// Filter by any of the transaction kinds
    TransactionKindIn(Vec<IotaTransactionKind>),
}

/// Transaction kinds for filtering
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IotaTransactionKind {
    ProgrammableTransaction,
    SystemTransaction,
    Genesis,
    ConsensusCommitPrologueV1,
    AuthenticatorStateUpdateV1,
    RandomnessStateUpdate,
    EndOfEpochTransaction,
}

impl From<&TransactionKind> for IotaTransactionKind {
    fn from(kind: &TransactionKind) -> Self {
        match kind {
            TransactionKind::ProgrammableTransaction(_) => Self::ProgrammableTransaction,
            TransactionKind::Genesis(_) => Self::Genesis,
            TransactionKind::ConsensusCommitPrologueV1(_) => Self::ConsensusCommitPrologueV1,
            TransactionKind::AuthenticatorStateUpdateV1(_) => Self::AuthenticatorStateUpdateV1,
            TransactionKind::RandomnessStateUpdate(_) => Self::RandomnessStateUpdate,
            TransactionKind::EndOfEpochTransaction(_) => Self::EndOfEpochTransaction,
        }
    }
}

impl Filter<EffectsWithInput> for TransactionFilter {
    fn matches(&self, item: &EffectsWithInput) -> bool {
        match self {
            TransactionFilter::MoveFunction {
                package,
                module,
                function,
            } => item.input.move_calls().into_iter().any(|(p, m, f)| {
                p == package
                    && (module.is_none() || matches!(module, Some(m2) if m2 == &m.to_string()))
                    && (function.is_none() || matches!(function, Some(f2) if f2 == &f.to_string()))
            }),
            TransactionFilter::InputObject(o) => {
                let Ok(input_objects) = item.input.input_objects() else {
                    return false;
                };
                input_objects.iter().any(|object| object.object_id() == *o)
            }
            TransactionFilter::ChangedObject(o) => {
                item.effects
                    .mutated()
                    .iter()
                    .any(|(obj_ref, _owner)| &obj_ref.0 == o)
            }
            TransactionFilter::FromAddress(a) => &item.input.sender() == a,
            TransactionFilter::ToAddress(a) => {
                item.effects
                    .mutated()
                    .iter()
                    .chain(item.effects.unwrapped().iter())
                    .any(|(_obj_ref, owner)| {
                        matches!(owner, iota_types::object::Owner::AddressOwner(addr) if *addr == *a)
                    })
            }
            TransactionFilter::FromAndToAddress { from, to } => {
                Self::FromAddress(*from).matches(item) && Self::ToAddress(*to).matches(item)
            }
            TransactionFilter::FromOrToAddress { addr } => {
                Self::FromAddress(*addr).matches(item) || Self::ToAddress(*addr).matches(item)
            }
            TransactionFilter::TransactionKind(kind) => {
                kind == &IotaTransactionKind::from(item.input.kind())
            }
            TransactionFilter::TransactionKindIn(kinds) => kinds
                .iter()
                .any(|kind| kind == &IotaTransactionKind::from(item.input.kind())),
        }
    }
}
