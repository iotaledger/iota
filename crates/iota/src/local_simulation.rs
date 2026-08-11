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
use iota_protocol_config::ProtocolConfig;
use iota_sdk::wallet_context::WalletContext;
use iota_sdk_types::{Address, ObjectReference, Transaction, TransactionKind};
use iota_types::{
    effects::TransactionEffectsAPI, gas::get_gas_balance, transaction::TransactionAPI,
};
use iota_vm_sdk::{ChainContext, ExecuteOptions, ExecutionResult, LocalVm, grpc::GrpcStore};

use crate::client_commands::{IotaClientCommandResult, cap_gas_budget_to_balance};

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
                cap_gas_budget_to_balance(balance, max_gas_budget)
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
    let object_changes = result
        .object_changes()?
        .into_iter()
        .map(Into::into)
        .collect();
    let balance_changes = result
        .balance_changes()?
        .into_iter()
        .map(Into::into)
        .collect();
    let raw_events = result.events.take().unwrap_or_default();

    let ExecutionResult {
        transaction,
        effects,
        output_objects,
        suggested_gas_price,
        ..
    } = result;
    // A dry run is not committed, so a package it published lives only in
    // `output_objects`; the resolver reads those before the store.
    let module_cache = vm.module_cache(&output_objects);

    let events = IotaTransactionBlockEvents::try_from_using_module_resolver(
        raw_events,
        tx_digest,
        None,
        &module_cache,
    )?;
    let input = IotaTransactionBlockData::try_from_with_module_cache(
        transaction,
        &module_cache,
        tx_digest,
    )?;

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
