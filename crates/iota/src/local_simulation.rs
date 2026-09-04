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

use anyhow::{Context, Result, anyhow};
use iota_json_rpc_types::{
    DryRunTransactionBlockResponse, IotaTransactionBlockData, IotaTransactionBlockEvents,
};
use iota_sdk::wallet_context::WalletContext;
use iota_sdk_types::{Address, ObjectReference, Transaction, TransactionKind};
use iota_types::{
    effects::TransactionEffectsAPI, gas::get_gas_balance, transaction::TransactionAPI,
};
use iota_vm_sdk::{ExecuteOptions, ExecutionResult, LocalVm, grpc::GrpcStore};

use crate::client_commands::{IotaClientCommandResult, fallback_gas_budget};

/// Build a [`LocalVm`] resolving objects on demand from the active env's gRPC
/// endpoint.
///
/// Object resolution blocks the calling thread, so this needs a multi-threaded
/// Tokio runtime.
async fn local_vm_from_context(context: &WalletContext) -> Result<LocalVm> {
    let client = context.get_grpc_client().await.context(
        "local simulation needs a gRPC endpoint; set `grpc` for the active env in client.yaml",
    )?;
    let store = GrpcStore::new(client);
    let chain_context = store.fetch_chain_context().await?;
    Ok(LocalVm::new(chain_context, store)?)
}

/// Run a dry-run locally and assemble the node-shaped response.
///
/// Needs a multi-threaded Tokio runtime, since resolving an object the run
/// asks for blocks the calling thread.
pub(crate) async fn execute_local_dry_run(
    context: &mut WalletContext,
    signer: Address,
    kind: TransactionKind,
    gas_budget: Option<u64>,
    gas_price: u64,
    gas_payment: Vec<ObjectReference>,
    sponsor: Option<Address>,
) -> Result<IotaClientCommandResult> {
    let mut vm = local_vm_from_context(context).await?;

    let gas_budget = match gas_budget {
        Some(gas_budget) => gas_budget,
        // The same fallback as the node-backed path, resolved from the
        // protocol config and the store instead of RPC calls.
        None => {
            let payment_balance = if gas_payment.is_empty() {
                None
            } else {
                let mut balance = 0;
                for object_ref in &gas_payment {
                    let coin = vm
                        .store()
                        .get_object(&object_ref.object_id, None)?
                        .ok_or_else(|| anyhow!("gas coin {} not found", object_ref.object_id))?;
                    balance += get_gas_balance(&coin)?;
                }
                Some(balance)
            };
            fallback_gas_budget(payment_balance, vm.protocol_config().max_tx_gas())
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

    let result = vm.execute(tx_data, ExecuteOptions::dry_run())?;
    let response = dry_run_response(&vm, result)?;
    Ok(IotaClientCommandResult::DryRun(response)
        .prerender_clever_errors(context)
        .await)
}

/// Assemble a [`DryRunTransactionBlockResponse`] from a local run, resolving
/// Move layouts from the packages the run wrote and those in the VM's store.
fn dry_run_response(
    vm: &LocalVm,
    mut result: ExecutionResult,
) -> Result<DryRunTransactionBlockResponse> {
    let tx_digest = *result.effects.transaction_digest();

    let execution_error_source = result
        .execution_error
        .as_ref()
        .and_then(|error| error.source().as_ref().map(|source| source.to_string()));
    // Both change sets are derived from the effects, so they come out in a
    // different order than the node reports them in. Consumers that compare
    // the two backends have to sort first.
    let object_changes = result
        .object_changes()?
        .into_iter()
        .map(TryInto::try_into)
        .collect::<Result<_, _>>()?;
    let balance_changes = result
        .balance_changes()?
        .into_iter()
        .map(Into::into)
        .collect();
    let raw_events = result.events.take().unwrap_or_default();

    // A dry run is not committed, so a package it published is not in the
    // store; the run's resolver reads it from the run's output first.
    let resolver = result.module_resolver(vm);
    let events = IotaTransactionBlockEvents::try_from_using_module_resolver(
        raw_events, tx_digest, None, &resolver,
    )?;
    let input = IotaTransactionBlockData::try_from_with_module_cache(
        result.transaction.clone(),
        &resolver,
        tx_digest,
    )?;

    let ExecutionResult {
        effects,
        suggested_gas_price,
        ..
    } = result;

    Ok(DryRunTransactionBlockResponse {
        effects: effects.try_into()?,
        events,
        object_changes,
        balance_changes,
        input,
        suggested_gas_price,
        execution_error_source,
    })
}
