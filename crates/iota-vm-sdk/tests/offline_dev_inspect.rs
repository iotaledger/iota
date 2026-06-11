// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Offline dev-inspect flow against the public `iota-vm-sdk` API.
//!
//! Mirrors the `offline_dev_inspect` example, but asserts the contract: a
//! `DevInspect` run succeeds against a framework-only store
//! with zero network access, returns per-command results, and leaves the store
//! untouched (`committed == false`). Also exercises the static
//! [`decode_transaction`] free function and confirms it agrees with the parsed
//! [`TransactionData`].

use base64::Engine;
use iota_sdk_types::ObjectId;
use iota_types::transaction::{TransactionData, TransactionDataAPI};
use iota_vm_sdk::{
    Chain, ChainContext, ExecuteOptions, ExecutionMode, InMemoryStore, LocalVm, ProtocolVersion,
    SignatureStatus, decode_transaction,
};

/// Base64-encoded BCS for `0x2::hash::blake2b256([0, 1, 2])` — a pure function
/// whose only dependencies are the framework packages, so it runs against a
/// framework-only store with no extra objects.
const BLAKE2B_TX_B64: &str = "AAABAAQDAAECAQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAgRoYXNoCmJsYWtlMmIyNTYAAQEAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA6AMAAAAAAAAAypo7AAAAAAA=";

fn blake2b_tx_bytes() -> Vec<u8> {
    base64::engine::general_purpose::STANDARD
        .decode(BLAKE2B_TX_B64)
        .expect("base64 decode")
}

fn chain_context() -> ChainContext {
    // Dev-inspect does not need these to match any real network state.
    ChainContext::new(ProtocolVersion::MAX, 1000, 0, 0, Chain::Unknown)
}

#[test]
fn dev_inspect_runs_offline_and_leaves_store_unchanged() {
    let tx_bytes = blake2b_tx_bytes();
    let tx: TransactionData = bcs::from_bytes(&tx_bytes).expect("decode tx");

    let store = InMemoryStore::with_framework();
    let objects_before = store.len();
    assert!(objects_before > 0, "framework store must not be empty");

    let mut vm = LocalVm::new(chain_context(), store).expect("build LocalVm");

    let result = vm
        .execute(tx, ExecuteOptions::dev_inspect())
        .expect("dev-inspect must succeed");

    assert!(
        result.status.is_success(),
        "blake2b256 dev-inspect must succeed, got {:?}",
        result.status
    );
    // One PTB command (the `blake2b256` Move call).
    assert_eq!(result.command_results.len(), 1);
    // Unsigned run: no signature was ever requested.
    assert!(matches!(
        result.signature_status,
        SignatureStatus::NotChecked
    ));
    // Dev-inspect is read-only: nothing committed, no debug captured.
    assert!(!result.committed, "dev-inspect must not commit");
    assert!(result.debug.is_none());
    // Gas was mocked because the transaction carries no gas payment.
    assert!(result.mock_gas_id.is_some());

    // The store the VM holds is untouched by a dev-inspect run (the mock gas
    // coin is never persisted).
    let store_after: Vec<_> = vm
        .store_mut()
        .get_object(&result.mock_gas_id.expect("mock gas id"), None)
        .into_iter()
        .collect();
    assert!(
        store_after.is_empty(),
        "mock gas coin must not be persisted into the store"
    );
}

#[test]
fn decode_transaction_matches_parsed_transaction_data() {
    let tx_bytes = blake2b_tx_bytes();
    let parsed: TransactionData = bcs::from_bytes(&tx_bytes).expect("decode tx");

    let decoded = decode_transaction(&tx_bytes).expect("decode_transaction");

    assert_eq!(decoded.sender, parsed.sender());
    assert_eq!(decoded.gas_budget, parsed.gas_budget());
    assert_eq!(decoded.gas_price, parsed.gas_price());

    // `required_objects` must agree with the input objects the typed parse
    // reports (sorted + de-duplicated). For this pure call that is just the
    // framework package hosting the `hash` module.
    let mut expected: Vec<ObjectId> = parsed
        .input_objects()
        .expect("input objects")
        .iter()
        .map(|kind| kind.object_id())
        .collect();
    expected.sort();
    expected.dedup();
    assert_eq!(decoded.required_objects, expected);
    assert!(
        !decoded.required_objects.is_empty(),
        "the framework package the call targets must be listed"
    );
}

#[test]
fn execute_options_constructors_set_the_mode() {
    assert_eq!(
        ExecuteOptions::dev_inspect().mode,
        ExecutionMode::DevInspect
    );
    assert_eq!(ExecuteOptions::dry_run().mode, ExecutionMode::DryRun);
    assert_eq!(ExecuteOptions::execute().mode, ExecutionMode::Execute);
}
