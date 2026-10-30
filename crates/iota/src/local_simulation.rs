// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Run transaction simulations locally through [`iota_vm_sdk::LocalVm`]
//! instead of a node's dry-run endpoint.
//!
//! Objects and chain parameters are resolved on demand from the active env's
//! gRPC endpoint; execution itself happens in-process, against the same Move
//! engine a node uses. The result is assembled into the same
//! [`DryRunTransactionBlockResponse`] the node returns, so it renders through
//! the same display code.
//!
//! The gas price defaults to the reference gas price the gRPC endpoint
//! reports, and a Move abort is rendered from the packages the run fetched,
//! so the run needs no JSON-RPC.
//!
//! Two checks a validator applies are out of reach here, so a transaction a
//! node's dry run rejects can still succeed locally: the operator's
//! transaction deny-list, and the network's signing verifier limits — this
//! runs with an empty deny-list and the default limits.

use std::sync::Arc;

use anyhow::{Context, Result};
use async_trait::async_trait;
use iota_json_rpc_types::{
    DryRunTransactionBlockResponse, IotaExecutionStatus, IotaTransactionBlockData,
    IotaTransactionBlockEffects, IotaTransactionBlockEffectsAPI, IotaTransactionBlockEvents,
};
use iota_package_resolver::{
    Package, PackageStore, Resolver, error::Error as PackageResolverError,
};
use iota_sdk::wallet_context::WalletContext;
use iota_sdk_types::{
    Address, ExecutionStatus, ObjectId, ObjectReference, Transaction, TransactionKind,
};
use iota_types::{
    effects::TransactionEffectsAPI, gas::report_simulation_gas, transaction::TransactionAPI,
};
use iota_vm_sdk::{
    ChainContext, ExecuteOptions, ExecutionResult, InMemoryStore, LocalVm, Store, grpc::GrpcStore,
};

use crate::{
    client_commands::{IotaClientCommandResult, estimate_gas_budget_from_gas_cost},
    displays::DryRunOutput,
};

/// Run a dry run locally and return the same
/// [`DryRunTransactionBlockResponse`] a node returns.
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
    let (store, chain_context) = connect(context).await?;
    let vm = LocalVm::new(chain_context, store.clone())?;
    let (mut response, status) = run_dry_run(
        vm,
        signer,
        kind,
        gas_budget,
        gas_price,
        gas_payment,
        sponsor,
    )?;

    // Render a Move abort as the node does, from the packages the run fetched.
    // Like the node, this cannot see a package the run itself published.
    let resolver = Resolver::new(FetchedPackages(store.store()));
    let IotaTransactionBlockEffects::V1(effects) = &mut response.effects;
    effects.status = IotaExecutionStatus::from_native_with_clever_error(status, &resolver).await?;
    Ok(IotaClientCommandResult::DryRun(DryRunOutput {
        response,
        local: true,
    }))
}

/// Estimate a gas budget from a local dry run, as
/// [`estimate_gas_budget`](crate::client_commands::estimate_gas_budget) does
/// from a node's.
pub(crate) async fn estimate_local_gas_budget(
    context: &WalletContext,
    signer: Address,
    kind: TransactionKind,
) -> Result<u64> {
    let (store, chain_context) = connect(context).await?;
    let reference_gas_price = chain_context.reference_gas_price;
    let vm = LocalVm::new(chain_context, store)?;
    let (response, _) = run_dry_run(vm, signer, kind, None, None, Vec::new(), None)?;
    Ok(estimate_gas_budget_from_gas_cost(
        response.effects.gas_cost_summary(),
        reference_gas_price,
    ))
}

async fn connect(context: &WalletContext) -> Result<(GrpcStore, ChainContext)> {
    let client = context.get_grpc_client().await.context(
        "local simulation needs a gRPC endpoint; set `grpc` for the active env in client.yaml",
    )?;
    let store = GrpcStore::new(client);
    let chain_context = store.fetch_chain_context().await?;
    Ok((store, chain_context))
}

struct FetchedPackages(InMemoryStore);

#[async_trait]
impl PackageStore for FetchedPackages {
    async fn fetch(&self, id: Address) -> Result<Arc<Package>, PackageResolverError> {
        let id = ObjectId::new(id.into_bytes());
        match self.0.get_object(&id, None) {
            Ok(Some(object)) => Package::read_from_object(&object).map(Arc::new),
            _ => Err(PackageResolverError::PackageNotFound(id.into())),
        }
    }
}

fn run_dry_run(
    mut vm: LocalVm,
    signer: Address,
    kind: TransactionKind,
    gas_budget: Option<u64>,
    gas_price: Option<u64>,
    gas_payment: Vec<ObjectReference>,
    sponsor: Option<Address>,
) -> Result<(DryRunTransactionBlockResponse, ExecutionStatus)> {
    let gas_price = gas_price.unwrap_or_else(|| vm.reference_gas_price());

    let tx_data = Transaction::new_with_gas_coins_allow_sponsor(
        kind,
        signer,
        gas_payment,
        gas_budget.unwrap_or_default(),
        gas_price,
        sponsor.unwrap_or(signer),
    );

    let result = vm.execute(tx_data.clone(), ExecuteOptions::dry_run())?;
    let status = result.effects.status().clone();
    Ok((dry_run_response(&vm, tx_data, result)?, status))
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
