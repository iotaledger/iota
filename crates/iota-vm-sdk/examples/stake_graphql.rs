// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Example: build a staking transaction with the transaction builder and
//! dry-run it locally against objects pre-fetched over GraphQL.
//!
//! The transaction is never signed: the builder resolves it against testnet,
//! its input objects (plus the system-state dynamic fields staking reads) are
//! pulled into a [`GraphqlStore`], and the run is a local
//! [`ExecutionMode::DryRun`] — nothing is submitted to the network.
//!
//! Run with:
//!   cargo run -p iota-vm-sdk --features graphql --example stake_graphql
//!
//! [`SENDER`] must own at least one IOTA coin on testnet; replace it with your
//! own funded address.

use anyhow::{Context, Result};
use iota_sdk_graphql_client::Client;
use iota_sdk_transaction_builder::TransactionBuilder;
use iota_sdk_types::Address;
use iota_vm_sdk::{ExecuteOptions, LocalVm, TransactionData, graphql::GraphqlStore};

/// Testnet GraphQL endpoint the store fetches from (matches
/// `Client::new_testnet`).
const TESTNET_GRAPHQL_URL: &str = "https://graphql.testnet.iota.cafe";
/// Account that pays for gas and provides the stake. Replace with your own
/// funded testnet address — it must own at least one IOTA coin.
const SENDER: &str = "0xda1820edf693ee32b5729907b9b2ec8e64980ee8c008c17e89cfb4e5ecd72151";
/// An active testnet validator to stake with.
const VALIDATOR: &str = "0x4f9791e5fbbcdf95b7e3c4f12da1f3c0d1c4d8c0d3f2e1a09876543210fedcba";
/// Amount to stake, in nanos (1 IOTA = 1_000_000_000 nanos).
const STAKE_AMOUNT: u64 = 1_000_000_000;

#[tokio::main]
async fn main() -> Result<()> {
    let sender: Address = SENDER.parse().context("parse sender address")?;
    let validator: Address = VALIDATOR.parse().context("parse validator address")?;

    // Build the staking transaction with the transaction builder over testnet
    // GraphQL. `stake` splits the stake amount off the gas coin, so the sender
    // only needs IOTA coins to pay for gas; `finish` selects them, estimates the
    // gas budget, and returns the transaction unsigned.
    let client = Client::new_testnet();
    let mut builder = TransactionBuilder::new(sender).with_client(client);
    builder.stake(STAKE_AMOUNT, validator);
    let tx: TransactionData = builder.finish().await.context("resolve staking tx")?;

    // Pull everything the local VM needs over GraphQL: the chain context, the
    // transaction's input objects, and the system-state dynamic fields the
    // staking call reads.
    let mut store = GraphqlStore::connect(TESTNET_GRAPHQL_URL).context("connect GraphQL store")?;
    let ctx = store
        .fetch_chain_context()
        .await
        .context("fetch chain context")?;
    store
        .prefetch(&tx)
        .await
        .context("prefetch input objects")?;
    store
        .prefetch_dynamic_fields()
        .await
        .context("prefetch dynamic fields")?;

    // Dry-run locally — no signature, no submission.
    let mut vm = LocalVm::new(ctx, store).context("build LocalVm")?;
    let result = vm
        .execute(tx, ExecuteOptions::dry_run())
        .context("local dry-run")?;

    println!("Staking dry-run status: {:?}", result.status);
    println!("Gas summary:            {:?}", result.gas_summary);
    Ok(())
}
