// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use async_graphql::OneofObject;
use iota_indexer::models::{events::StoredEvent, transactions::StoredTransaction};
use iota_json_rpc_types::{Filter, IotaTransactionKind};
use iota_types::{base_types::ObjectID, transaction::TransactionDataAPI};

use crate::types::{
    iota_address::IotaAddress,
    transaction_block::TransactionBlockKindInput,
    type_filter::{FqNameFilter, ModuleFilter},
};

/// Filter incoming events in a subscription.
#[derive(OneofObject, Clone, Debug, Eq, PartialEq)]
pub(crate) enum SubscriptionEventFilter {
    /// Filter incoming events by emitting module.
    ///
    /// - Filter by package: "0x02"
    /// - Filter by module: "0x02::coin"
    EmittingModule(ModuleFilter),
}

impl Filter<StoredEvent> for SubscriptionEventFilter {
    fn matches(&self, event: &StoredEvent) -> bool {
        use SubscriptionEventFilter::*;
        match self {
            EmittingModule(ModuleFilter::ByPackage(package)) => {
                event.package.as_slice() == package.as_slice()
            }
            EmittingModule(ModuleFilter::ByModule(package, module)) => {
                event.package.as_slice() == package.as_slice() && event.module == *module
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
    ///
    /// - Filter by package: "0x03"
    /// - Filter by module: "0x03::iota_system"
    /// - Filter by function: "0x03::iota_system::request_add_stake"
    Function(FqNameFilter),
}

impl Filter<StoredTransaction> for SubscriptionTransactionFilter {
    fn matches(&self, transaction: &StoredTransaction) -> bool {
        use SubscriptionTransactionFilter::*;
        match self {
            Kind(kind) => transaction.transaction_kind == IotaTransactionKind::from(kind) as i16,
            SigningAddress(address) => transaction
                .try_into_sender_signed_data()
                .map(|data| data.transaction_data().sender() == (*address).into())
                .unwrap_or_default(),
            Function(name) => {
                let move_call = MoveCall::from(name);

                transaction
                    .try_into_sender_signed_data()
                    .map(|data| {
                        data.transaction_data()
                            .move_calls()
                            .iter()
                            .any(|(p, m, f)| move_call.matches_transaction_move_call(p, m, f))
                    })
                    .unwrap_or_default()
            }
        }
    }
}

impl From<&TransactionBlockKindInput> for IotaTransactionKind {
    fn from(value: &TransactionBlockKindInput) -> Self {
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

/// Represents a module information of the move call.
struct ModuleFunction<'a> {
    /// Name of the module.
    module_name: &'a str,
    /// Name of the function within the module.
    function_name: Option<&'a str>,
}

/// A data type that converts [`FqNameFilter`] into a representation of move
/// calls in transactions, enabling easy filtering by fully qualified names as
/// returned by the [`move_calls`](TransactionDataAPI::move_calls) method on
/// types implementing [`TransactionDataAPI`].
struct MoveCall<'a> {
    /// Package ID of the move call.
    package: ObjectID,
    /// Module information of the move call.
    module_function: Option<ModuleFunction<'a>>,
}

impl<'a> MoveCall<'a> {
    fn new(package: ObjectID, module_function: impl Into<Option<ModuleFunction<'a>>>) -> Self {
        MoveCall {
            package,
            module_function: module_function.into(),
        }
    }

    /// Matches a transaction move call against the fully qualified name filter.
    ///
    /// The filter is applied in the following order:
    /// 1. package ID must match
    /// 2. if a module name is specified, it must match
    /// 3. if a function name is specified, it must match
    fn matches_transaction_move_call(
        &self,
        package: &ObjectID,
        module: &str,
        function: &str,
    ) -> bool {
        self.package == *package
            && self.module_function.as_ref().is_none_or(|mf| {
                mf.module_name == module && mf.function_name.is_none_or(|f| f == function)
            })
    }
}

impl<'a> From<&'a FqNameFilter> for MoveCall<'a> {
    fn from(value: &'a FqNameFilter) -> Self {
        use FqNameFilter::*;
        match value {
            ByModule(ModuleFilter::ByPackage(package)) => Self::new((*package).into(), None),
            ByModule(ModuleFilter::ByModule(package, module_name)) => Self::new(
                (*package).into(),
                ModuleFunction {
                    module_name,
                    function_name: None,
                },
            ),
            ByFqName(package, module_name, function_name) => Self::new(
                (*package).into(),
                ModuleFunction {
                    module_name,
                    function_name: Some(function_name),
                },
            ),
        }
    }
}
