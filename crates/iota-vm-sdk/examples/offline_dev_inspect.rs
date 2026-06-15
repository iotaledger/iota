// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Example: fully offline dev-inspect of a Move transaction.
//!
//! Runs the Move VM with zero network access. The framework packages are
//! provided by [`InMemoryStore::with_framework`]; the transaction calls
//! `0x2::hash::blake2b256([0, 1, 2])`, a pure function whose only dependencies
//! are the framework packages.
//!
//! Usage: `cargo run -p iota-vm-sdk --example offline_dev_inspect`

use anyhow::Result;
use iota_types::{
    effects::TransactionEffectsAPI,
    transaction::{TransactionData, TransactionDataAPI},
};
use iota_vm_sdk::{Chain, ChainContext, ExecuteOptions, InMemoryStore, LocalVm, ProtocolVersion};

fn main() -> Result<()> {
    // Framework packages (0x1, 0x2, …) are compiled into the binary.
    let store = InMemoryStore::with_framework();

    let ctx = ChainContext::new(ProtocolVersion::MAX, 1000, 0, 0, Chain::Unknown);
    let mut vm = LocalVm::new(ctx, store)?;

    // Base64-encoded BCS for: 0x2::hash::blake2b256([0, 1, 2]).
    let tx_b64 = "AAABAAQDAAECAQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAgRoYXNoCmJsYWtlMmIyNTYAAQEAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA6AMAAAAAAAAAypo7AAAAAAA=";
    let tx_bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, tx_b64)?;
    let tx: TransactionData = bcs::from_bytes(&tx_bytes)?;
    println!("Sender:    {}", tx.sender());

    let result = vm.execute(tx, ExecuteOptions::dev_inspect())?;

    println!("Result:  {result:?}");
    println!("Status:  {:?}", result.effects.status());
    println!("Committed: {}", result.committed);
    println!("Commands:  {}", result.command_results.len());
    for (i, (mut_refs, returns)) in result.command_results.iter().enumerate() {
        println!(
            "  [{i}] mutable_ref_outputs={}, return_values={}",
            mut_refs.len(),
            returns.len()
        );
    }
    Ok(())
}
