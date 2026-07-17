// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Example: build a staking transaction with the transaction builder and
//! dry-run it locally against objects resolved on demand over gRPC.
//!
//! The transaction is never signed: the builder resolves it against testnet,
//! its input objects (plus the system-state dynamic fields staking reads) are
//! resolved on demand from a [`GrpcStore`], and the run is a local
//! [`ExecutionMode::DryRun`] — nothing is submitted to the network.
//!
//! Run with:
//!   cargo run -p iota-vm-sdk --features grpc --example stake_grpc

use anyhow::{Context, Result};
use iota_grpc_client::Client;
use iota_sdk_transaction_builder::TransactionBuilder;
use iota_sdk_types::Address;
use iota_vm_sdk::{ExecuteOptions, LocalVm, TransactionData, grpc::GrpcStore};

#[tokio::main]
async fn main() -> Result<()> {
    // Account that pays for gas and provides the stake.
    let sender =
        Address::from_hex("0xda1820edf693ee32b5729907b9b2ec8e64980ee8c008c17e89cfb4e5ecd72151")?;
    // An active testnet validator to stake with.
    let validator =
        Address::from_hex("0xa276b4c076fff55588255630e9ee35cf0d07e8d80c78991cfd58b43b687b4206")?;
    let stake_amount_nanos: u64 = 1_000_000_000;

    // Build the staking transaction.
    let client = Client::new_testnet().context("connect testnet gRPC client")?;
    let mut builder = TransactionBuilder::new(sender).with_client(client.clone());
    builder.stake(stake_amount_nanos, validator);
    let tx: TransactionData = builder.finish().await.context("resolve staking tx")?;

    // The store resolves every object the VM reads over gRPC on demand —
    // inputs and the system-state dynamic fields staking walks — so only the
    // chain context is fetched up front.
    let store = GrpcStore::new(client);
    let ctx = store
        .fetch_chain_context()
        .await
        .context("fetch chain context")?;

    // Dry-run locally — no signature, no submission.
    let mut vm = LocalVm::new(ctx, store).context("build LocalVm")?;
    let result = vm
        .execute(tx, ExecuteOptions::dry_run())
        .context("local dry-run")?;

    println!("Staking dry-run status: {:?}", result.status);
    println!("Gas summary:            {:?}", result.gas_summary);
    Ok(())
}
