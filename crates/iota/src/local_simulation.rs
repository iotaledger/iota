// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Run transaction simulations locally through [`iota_vm_sdk::LocalVm`]
//! instead of a node's dry-run endpoint.
//!
//! Objects and chain parameters are resolved on demand from the active env's
//! gRPC endpoint; execution itself happens in-process, against the same Move
//! engine a node uses. The result is assembled into the same
//! [`DryRunTransactionBlockResponse`] the node returns, so the rendered output
//! matches the node-backed path.

use std::{cmp::min, collections::BTreeMap};

use anyhow::{Context, Result, anyhow};
use colored::Colorize;
use iota_json_rpc_types::{
    BalanceChange, DryRunTransactionBlockResponse, IotaTransactionBlockData,
    IotaTransactionBlockEvents, ObjectChange,
};
use iota_protocol_config::ProtocolConfig;
use iota_sdk::wallet_context::WalletContext;
use iota_sdk_types::{
    Address, ObjectId, ObjectReference, StructTag, Transaction, TransactionKind, Version,
};
use iota_types::{
    effects::{ObjectRemoveKind, TransactionEffectsAPI, TransactionEffectsExt},
    error::IotaResult,
    gas::get_gas_balance,
    object::Object,
    storage::{
        BackingPackageStore, ObjectStore, PackageObject, WriteKind, error::Error as StorageError,
    },
    transaction::TransactionAPI,
};
use iota_vm_sdk::{ChainContext, ExecuteOptions, ExecutionResult, LocalVm, Store, grpc::GrpcStore};
use move_bytecode_utils::module_cache::GetModule;
use move_core_types::language_storage::ModuleId;

use crate::client_commands::IotaClientCommandResult;

/// Build a [`LocalVm`] resolving objects on demand from the active env's gRPC
/// endpoint. Also returns the fetched [`ChainContext`] so callers can read the
/// chain parameters the VM was built for.
pub async fn local_vm_from_context(context: &WalletContext) -> Result<(LocalVm, ChainContext)> {
    let client = context.get_grpc_client().await.context(
        "local simulation needs a gRPC endpoint; set `grpc` for the active env in client.yaml",
    )?;
    let store = GrpcStore::new(client);
    let chain_context = store.fetch_chain_context().await?;
    Ok((LocalVm::new(chain_context.clone(), store)?, chain_context))
}

/// Run a dry-run locally and assemble the node-shaped response.
pub async fn execute_local_dry_run(
    context: &mut WalletContext,
    signer: Address,
    kind: TransactionKind,
    gas_budget: Option<u64>,
    gas_price: u64,
    gas_payment: Vec<ObjectReference>,
    sponsor: Option<Address>,
) -> Result<IotaClientCommandResult> {
    let (mut vm, chain_context) = local_vm_from_context(context).await?;

    let gas_budget = match gas_budget {
        // Mirrors the node-backed path's fallback: the protocol's maximum,
        // capped at the total balance of any provided gas coins — resolved
        // from the protocol config and the store instead of RPC calls.
        Some(gas_budget) => gas_budget,
        None => {
            let max_gas_budget = ProtocolConfig::get_for_version(
                chain_context.protocol_version,
                chain_context.chain,
            )
            .max_tx_gas();
            if gas_payment.is_empty() {
                max_gas_budget
            } else {
                let mut balance = 0;
                for object_ref in &gas_payment {
                    let coin = vm
                        .store()
                        .get_object(&object_ref.object_id, None)?
                        .ok_or_else(|| anyhow!("gas coin {} not found", object_ref.object_id))?;
                    balance += get_gas_balance(&coin)?;
                }
                let gas_budget = min(balance, max_gas_budget);
                if gas_budget == balance {
                    let warn_msg = format!(
                        "Gas budget is equal to the total gas balance of the provided gas coins: {balance}. Manually provide a lower --gas-budget if you need to split a coin from the gas coin."
                    );
                    eprintln!("{}", warn_msg.yellow().bold());
                }
                gas_budget
            }
        }
    };

    let tx_data = Transaction::new_with_gas_coins_allow_sponsor(
        kind,
        signer,
        gas_payment,
        gas_budget,
        gas_price,
        sponsor.unwrap_or(signer),
    );

    let result = vm.execute(tx_data.clone(), ExecuteOptions::dry_run())?;
    let response = dry_run_response(&vm, tx_data, result)?;
    Ok(IotaClientCommandResult::DryRun(response)
        .prerender_clever_errors(context)
        .await)
}

/// Assemble a [`DryRunTransactionBlockResponse`] from a local run, resolving
/// Move layouts from the packages held in the VM's store.
fn dry_run_response(
    vm: &LocalVm,
    tx_data: Transaction,
    result: ExecutionResult,
) -> Result<DryRunTransactionBlockResponse> {
    let tx_digest = *result.effects.transaction_digest();
    let module_cache = StoreModuleCache(vm.store());

    let execution_error_source = match &result.status {
        iota_sdk_types::ExecutionStatus::Failure { error, .. } => Some(format!("{error:?}")),
        _ => None,
    };
    let events = IotaTransactionBlockEvents::try_from_using_module_resolver(
        result.events.clone().unwrap_or_default(),
        tx_digest,
        None,
        &module_cache,
    )?;
    let object_changes = object_changes_from_result(tx_data.sender(), &result);
    let balance_changes = balance_changes_from_result(&result);
    let input =
        IotaTransactionBlockData::try_from_with_module_cache(tx_data, &module_cache, tx_digest)?;

    Ok(DryRunTransactionBlockResponse {
        effects: result.effects.try_into()?,
        events,
        object_changes,
        balance_changes,
        input,
        suggested_gas_price: None,
        execution_error_source,
    })
}

/// Compute the node-shaped object changes from a local run's effects and
/// object sets.
fn object_changes_from_result(sender: Address, result: &ExecutionResult) -> Vec<ObjectChange> {
    let output_objects: BTreeMap<ObjectId, &Object> =
        result.output_objects.iter().map(|o| (o.id(), o)).collect();
    let input_objects: BTreeMap<ObjectId, &Object> =
        result.input_objects.iter().map(|o| (o.id(), o)).collect();
    let modified_at: BTreeMap<ObjectId, Version> =
        result.effects.modified_at_versions().into_iter().collect();

    let mut changes = vec![];
    for (object_ref, owner, kind) in result.effects.all_changed_objects() {
        let ObjectReference {
            object_id,
            version,
            digest,
        } = object_ref;
        let Some(object) = output_objects.get(&object_id) else {
            continue;
        };
        if let Some(move_object_type) = object.type_() {
            let object_type: StructTag = move_object_type.clone().into();
            changes.push(match kind {
                WriteKind::Mutate => ObjectChange::Mutated {
                    sender,
                    owner,
                    object_type,
                    object_id,
                    version,
                    previous_version: modified_at.get(&object_id).copied().unwrap_or_default(),
                    digest,
                },
                WriteKind::Create => ObjectChange::Created {
                    sender,
                    owner,
                    object_type,
                    object_id,
                    version,
                    digest,
                },
                WriteKind::Unwrap => ObjectChange::Unwrapped {
                    sender,
                    owner,
                    object_type,
                    object_id,
                    version,
                    digest,
                },
            });
        } else if let Some(package) = object.data.as_opt_package() {
            if kind == WriteKind::Create {
                changes.push(ObjectChange::Published {
                    package_id: package.id(),
                    version: package.version(),
                    digest,
                    modules: package
                        .serialized_module_map()
                        .keys()
                        .map(|k| k.to_string())
                        .collect(),
                });
            }
        }
    }

    for (object_ref, kind) in result.effects.all_removed_objects() {
        let object_id = object_ref.object_id;
        let Some(object_type) = input_objects
            .get(&object_id)
            .and_then(|o| o.type_())
            .map(|t| StructTag::from(t.clone()))
        else {
            continue;
        };
        let version = object_ref.version;
        changes.push(match kind {
            ObjectRemoveKind::Delete => ObjectChange::Deleted {
                sender,
                object_type,
                object_id,
                version,
            },
            ObjectRemoveKind::Wrap => ObjectChange::Wrapped {
                sender,
                object_type,
                object_id,
                version,
            },
        });
    }

    changes
}

/// Compute the node-shaped balance changes by diffing coin values between the
/// run's input and output objects.
fn balance_changes_from_result(result: &ExecutionResult) -> Vec<BalanceChange> {
    let mut changes: BTreeMap<(iota_sdk_types::Owner, String), (iota_sdk_types::TypeTag, i128)> =
        BTreeMap::new();
    let mut record = |object: &Object, negate: bool| {
        let Some(coin) = object.as_coin_maybe() else {
            return;
        };
        let Some(object_type) = object.type_().filter(|t| t.is_coin()) else {
            return;
        };
        let coin_type = object_type.type_params()[0].clone();
        let amount = coin.balance.value() as i128;
        let entry = changes
            .entry((object.owner, coin_type.to_string()))
            .or_insert((coin_type, 0));
        entry.1 += if negate { -amount } else { amount };
    };
    for object in &result.input_objects {
        record(object, true);
    }
    for object in &result.output_objects {
        record(object, false);
    }
    changes
        .into_iter()
        .filter(|(_, (_, amount))| *amount != 0)
        .map(|((owner, _), (coin_type, amount))| BalanceChange {
            owner,
            coin_type,
            amount,
        })
        .collect()
}

/// Adapts the VM's [`Store`] to the [`GetModule`] interface the JSON-RPC type
/// conversions use to resolve Move layouts.
struct StoreModuleCache<'a>(&'a dyn Store);

impl ObjectStore for StoreModuleCache<'_> {
    fn try_get_object(&self, object_id: &ObjectId) -> Result<Option<Object>, StorageError> {
        self.0
            .get_object(object_id, None)
            .map_err(StorageError::custom)
    }

    fn try_get_object_by_key(
        &self,
        object_id: &ObjectId,
        version: Version,
    ) -> Result<Option<Object>, StorageError> {
        self.0
            .get_object(object_id, Some(version))
            .map_err(StorageError::custom)
    }
}

impl BackingPackageStore for StoreModuleCache<'_> {
    fn get_package_object(&self, package_id: &ObjectId) -> IotaResult<Option<PackageObject>> {
        iota_types::storage::load_package_object_from_object_store(self, package_id)
    }
}

impl GetModule for StoreModuleCache<'_> {
    type Error = iota_types::error::IotaError;
    type Item = move_binary_format::CompiledModule;

    fn get_module_by_id(&self, id: &ModuleId) -> Result<Option<Self::Item>, Self::Error> {
        iota_types::storage::get_module_by_id(self, id)
    }
}
