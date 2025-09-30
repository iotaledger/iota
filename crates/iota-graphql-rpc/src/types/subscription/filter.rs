// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use async_graphql::OneofObject;
use iota_indexer::stream::{ModuleFunction, StreamEventFilter, StreamTransactionFilter};
use iota_json_rpc_types::IotaTransactionKind;
use iota_types::base_types::ObjectID;

use crate::types::{
    iota_address::IotaAddress,
    transaction_block::TransactionBlockKindInput,
    type_filter::{FqNameFilter, ModuleFilter},
};

/// Filter incoming events in a subscription.
#[derive(OneofObject, Clone, Debug, Eq, PartialEq)]
pub(crate) enum SubscriptionEventFilter {
    /// Filter incoming events by emitting module.
    EmittingModule(ModuleFilter),
}

impl From<SubscriptionEventFilter> for StreamEventFilter {
    fn from(value: SubscriptionEventFilter) -> Self {
        use SubscriptionEventFilter::*;
        match value {
            EmittingModule(ModuleFilter::ByPackage(package)) => {
                StreamEventFilter::EmittingPackage(package.into())
            }
            EmittingModule(ModuleFilter::ByModule(package, module)) => {
                StreamEventFilter::EmittingModule {
                    package: package.into(),
                    module,
                }
            }
        }
    }
}

/// Filter incoming transactions in a subscription.
#[derive(OneofObject, Clone, Debug, Eq, PartialEq)]
pub(crate) enum SubscriptionTransactionFilter {
    /// Filter incoming transactions by kind.
    Kind(TransactionBlockKindInput),
    /// Filter incoming transactions by signing address.
    SigningAddress(IotaAddress),
    /// Filter incoming transactions by package, module, or function name.
    Function(FqNameFilter),
}

impl From<SubscriptionTransactionFilter> for StreamTransactionFilter {
    fn from(value: SubscriptionTransactionFilter) -> Self {
        use SubscriptionTransactionFilter::*;
        match value {
            Kind(kind) => StreamTransactionFilter::Kind(kind.into()),
            SigningAddress(address) => StreamTransactionFilter::SigningAddress(address.into()),
            Function(name) => {
                let (package, module) = name.into();
                StreamTransactionFilter::Function { package, module }
            }
        }
    }
}

impl From<TransactionBlockKindInput> for IotaTransactionKind {
    fn from(value: TransactionBlockKindInput) -> Self {
        match value {
            TransactionBlockKindInput::SystemTx => IotaTransactionKind::SystemTransaction,
            TransactionBlockKindInput::ProgrammableTx => {
                IotaTransactionKind::ProgrammableTransaction
            }
            TransactionBlockKindInput::Genesis => IotaTransactionKind::Genesis,
            TransactionBlockKindInput::ConsensusCommitPrologueV1 => {
                IotaTransactionKind::ConsensusCommitPrologueV1
            }
            TransactionBlockKindInput::AuthenticatorStateUpdateV1 => {
                IotaTransactionKind::AuthenticatorStateUpdateV1
            }
            TransactionBlockKindInput::RandomnessStateUpdate => {
                IotaTransactionKind::RandomnessStateUpdate
            }
            TransactionBlockKindInput::EndOfEpochTx => IotaTransactionKind::EndOfEpochTransaction,
        }
    }
}

impl From<FqNameFilter> for (ObjectID, Option<ModuleFunction>) {
    fn from(value: FqNameFilter) -> Self {
        use FqNameFilter::*;
        match value {
            ByModule(ModuleFilter::ByPackage(package)) => (package.into(), None),
            ByModule(ModuleFilter::ByModule(package, module_name)) => (
                package.into(),
                Some(ModuleFunction {
                    module_name,
                    function_name: None,
                }),
            ),
            ByFqName(package, module_name, function_name) => (
                package.into(),
                Some(ModuleFunction {
                    module_name,
                    function_name: Some(function_name),
                }),
            ),
        }
    }
}
