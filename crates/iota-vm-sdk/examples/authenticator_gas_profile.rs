// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Gas-profile a `MoveAuthenticator`-signed transaction with the local VM.
//!
//! A `MoveAuthenticator` runs in two metered VM invocations — the authenticator
//! function, then the PTB body — so the merged Speedscope document holds a
//! separate gas flamegraph for each.
//!
//! Usage: `cargo run -p iota-vm-sdk --example authenticator_gas_profile`.
//! Writes `authenticator_gas_profile.speedscope.json`; view it at
//! <https://www.speedscope.app>.

use std::path::PathBuf;

use anyhow::{Context, Result};
use base64::{Engine, engine::general_purpose::STANDARD};
use iota_types::{
    effects::TransactionEffectsAPI,
    object::Object,
    signature::GenericSignature,
    transaction::{SenderSignedData, TransactionData, TransactionDataAPI},
};
use iota_vm_sdk::{
    Chain, ChainContext, DebugConfig, ExecuteOptions, InMemoryStore, LocalVm, ProfileOutput,
    ProfileSink, ProtocolVersion, Store,
};
use serde_json::Value;

fn main() -> Result<()> {
    let out_path = PathBuf::from("authenticator_gas_profile.speedscope.json");

    let signed = example_signed_transaction()?;
    let sender = signed.transaction_data().sender();

    // Store: framework packages plus every object the run touches.
    let f = fixture()?;
    let mut store = InMemoryStore::with_framework();
    for obj in f["objects"]
        .as_array()
        .context("`objects` must be an array")?
    {
        let object: Object =
            bcs::from_bytes(&STANDARD.decode(obj["bcs_b64"].as_str().unwrap()).unwrap())?;
        store.insert(object);
    }

    let ctx = ChainContext::new(
        ProtocolVersion::new(f["protocol_version"].as_u64().context("protocol_version")?),
        Chain::Unknown,
    )
    .with_reference_gas_price(f["reference_gas_price"].as_u64().unwrap_or(0))
    .with_epoch_id(f["epoch_id"].as_u64().unwrap_or(0))
    .with_epoch_timestamp_ms(f["epoch_timestamp_ms"].as_u64().unwrap_or(0));

    println!("Sender:           {sender}");

    // --- Execute, with the gas profiler writing a Speedscope doc. The merged
    // doc carries one profile for the authenticator function and one for the
    // PTB body. ---
    let mut vm = LocalVm::new(ctx, store)?;
    let opts = ExecuteOptions::dry_run()
        .with_debug(DebugConfig::default().with_profile(ProfileSink::Path(out_path)));
    let result = vm.execute_signed(signed, opts)?;

    // --- Inspect: status, signature verdict, and the gas ledger. ---
    println!("Execution status: {:?}", result.effects.status());
    println!("Signature status: {:?}", result.signature_status);
    println!("Gas cost summary: {:?}", result.gas_summary);

    match result.debug.and_then(|d| d.profile) {
        Some(ProfileOutput::Path(p)) => {
            println!(
                "Gas flamegraph (Speedscope JSON) written to: {}",
                p.display()
            );
            println!("View it at https://www.speedscope.app or render it to an image.");
        }
        _ => println!("No gas profile was produced (the run metered no computation)."),
    }

    Ok(())
}

/// Read the committed fixture JSON.
fn fixture() -> Result<Value> {
    let path: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "tests",
        "fixtures",
        "move_auth_ed25519_valid.json",
    ]
    .iter()
    .collect();
    let raw = std::fs::read_to_string(&path)
        .with_context(|| format!("read fixture {}", path.display()))?;
    serde_json::from_str(&raw).context("parse fixture JSON")
}

/// Build the example's signed transaction — PTB body plus its MoveAuthenticator
/// signature — from the committed fixture.
fn example_signed_transaction() -> Result<SenderSignedData> {
    let f = fixture()?;
    let tx: TransactionData =
        bcs::from_bytes(&STANDARD.decode(f["tx_b64"].as_str().unwrap()).unwrap())?;
    let sigs: Vec<GenericSignature> = f["signatures"]
        .as_array()
        .context("`signatures` must be an array")?
        .iter()
        .map(|s| {
            Ok(GenericSignature::from_bytes(
                &STANDARD.decode(s.as_str().unwrap()).unwrap(),
            )?)
        })
        .collect::<Result<_>>()?;
    Ok(SenderSignedData::new(tx, sigs))
}
