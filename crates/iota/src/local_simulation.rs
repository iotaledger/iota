// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Run transaction simulations locally through [`iota_vm_sdk::LocalVm`]
//! instead of the node's dry-run and dev-inspect endpoints.
//!
//! Objects and chain parameters are resolved on demand from the active env's
//! gRPC endpoint; execution itself happens in-process, against the same Move
//! engine a node uses. The results are assembled into the same
//! [`DryRunTransactionBlockResponse`] / [`DevInspectResults`] the node
//! returns, so they render through the same display code.
//!
//! The gas price defaults to the reference gas price the gRPC endpoint
//! reports, so only rendering a Move abort still goes through JSON-RPC.
//!
//! Two checks a validator applies are out of reach in a dry run, so a
//! transaction a node's dry run rejects can still succeed locally: the
//! operator's transaction deny-list, and the network's signing verifier
//! limits — this runs with an empty deny-list and the default limits. A
//! dev-inspect skips those checks on a node by default, so the local one
//! matches.

use anyhow::{Context, Result, anyhow};
use iota_json_rpc_types::{
    DevInspectResults, DryRunTransactionBlockResponse, IotaTransactionBlockData,
    IotaTransactionBlockEvents,
};
use iota_sdk::wallet_context::WalletContext;
use iota_sdk_types::{Address, ObjectReference, Transaction, TransactionKind};
use iota_types::{
    effects::TransactionEffectsAPI,
    gas::{get_gas_balance, report_simulation_gas},
    transaction::TransactionAPI,
};
use iota_vm_sdk::{ExecuteOptions, ExecutionResult, LocalVm, grpc::GrpcStore};

use crate::client_commands::{IotaClientCommandResult, fallback_gas_budget};

/// Run a dry-run locally and assemble the node-shaped response.
///
/// The run blocks the calling thread until it is done. Resolving an object
/// the run asks for needs a multi-threaded Tokio runtime.
pub(crate) async fn execute_local_dry_run(
    context: &mut WalletContext,
    signer: Address,
    kind: TransactionKind,
    gas_budget: Option<u64>,
    gas_price: Option<u64>,
    gas_payment: Vec<ObjectReference>,
    sponsor: Option<Address>,
) -> Result<IotaClientCommandResult> {
    let vm = local_vm(context).await?;
    let response = run_dry_run(
        vm,
        signer,
        kind,
        gas_budget,
        gas_price,
        gas_payment,
        sponsor,
    )?;
    IotaClientCommandResult::LocalDryRun(response)
        .prerender_clever_errors(context)
        .await
}

/// Run a dev-inspect locally and assemble the node-shaped results.
///
/// The run blocks the calling thread until it is done. Resolving an object
/// the run asks for needs a multi-threaded Tokio runtime.
pub(crate) async fn execute_local_dev_inspect(
    context: &mut WalletContext,
    signer: Address,
    kind: TransactionKind,
    gas_budget: Option<u64>,
    gas_price: Option<u64>,
    gas_payment: Vec<ObjectReference>,
    sponsor: Option<Address>,
) -> Result<IotaClientCommandResult> {
    let vm = local_vm(context).await?;
    let results = run_dev_inspect(
        vm,
        signer,
        kind,
        gas_budget,
        gas_price,
        gas_payment,
        sponsor,
    )?;
    // The node's dev-inspect converts the effects as-is, so a Move abort is
    // reported raw, unlike a dry run; nothing to re-render through JSON-RPC.
    Ok(IotaClientCommandResult::LocalDevInspect(results))
}

async fn local_vm(context: &mut WalletContext) -> Result<LocalVm> {
    let client = context.get_grpc_client().await.context(
        "local simulation needs a gRPC endpoint; set `grpc` for the active env in client.yaml",
    )?;
    let store = GrpcStore::new(client);
    let chain_context = store.fetch_chain_context().await?;
    Ok(LocalVm::new(chain_context, store)?)
}

fn run_dry_run(
    mut vm: LocalVm,
    signer: Address,
    kind: TransactionKind,
    gas_budget: Option<u64>,
    gas_price: Option<u64>,
    gas_payment: Vec<ObjectReference>,
    sponsor: Option<Address>,
) -> Result<DryRunTransactionBlockResponse> {
    let gas_price = gas_price.unwrap_or_else(|| vm.reference_gas_price());

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

    let result = vm.execute(tx_data.clone(), ExecuteOptions::dry_run())?;
    dry_run_response(&vm, tx_data, result)
}

fn run_dev_inspect(
    mut vm: LocalVm,
    signer: Address,
    kind: TransactionKind,
    gas_budget: Option<u64>,
    gas_price: Option<u64>,
    gas_payment: Vec<ObjectReference>,
    sponsor: Option<Address>,
) -> Result<DevInspectResults> {
    // A zero budget or price is filled in by the run the same way the node's
    // dev-inspect fills it in, so neither is resolved here.
    let tx_data = Transaction::new_with_gas_coins_allow_sponsor(
        kind,
        signer,
        gas_payment,
        gas_budget.unwrap_or_default(),
        gas_price.unwrap_or_default(),
        sponsor.unwrap_or(signer),
    );
    let mut result = vm.execute(tx_data, ExecuteOptions::dev_inspect())?;

    let return_values = match result.execution_error.take() {
        Some(error) => Err(error),
        None => Ok(std::mem::take(&mut result.command_results)),
    };
    let events = result.events.take().unwrap_or_default();
    let effects = result.effects.clone();
    let mut layout_resolver = result.layout_resolver(&vm)?;
    // The CLI does not ask for the raw transaction or effects bytes.
    Ok(DevInspectResults::new(
        effects,
        events,
        return_values,
        vec![],
        vec![],
        layout_resolver.as_mut(),
    )?)
}

/// Assemble a [`DryRunTransactionBlockResponse`] from a local run, resolving
/// Move layouts from the packages the run wrote and those in the VM's store.
fn dry_run_response(
    vm: &LocalVm,
    mut tx_data: Transaction,
    mut result: ExecutionResult,
) -> Result<DryRunTransactionBlockResponse> {
    let tx_digest = *result.effects.transaction_digest();

    let execution_error_source = result
        .execution_error
        .as_ref()
        .and_then(|error| error.source().as_ref().map(|source| source.to_string()));
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
    // Report the gas the run used in place of whatever the caller left unset,
    // as the node does before it builds the response's input.
    report_simulation_gas(
        tx_data.gas_data_mut(),
        result.transaction.gas_data(),
        result.effects.gas_cost_summary().gas_used(),
    );
    let input =
        IotaTransactionBlockData::try_from_with_module_cache(tx_data, &resolver, tx_digest)?;

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
        // Congestion is only observable on a node, so where the SDK withholds
        // a suggestion report the reference gas price — what a node suggests
        // when nothing is congested.
        suggested_gas_price: Some(suggested_gas_price.unwrap_or_else(|| vm.reference_gas_price())),
        execution_error_source,
    })
}
