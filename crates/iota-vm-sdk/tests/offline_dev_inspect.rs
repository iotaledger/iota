// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Offline dev-inspect flow against the public `iota-vm-sdk` API.
//!
//! Mirrors the `offline_dev_inspect` example, but asserts the contract: a
//! `DevInspect` run succeeds against a framework-only store
//! with zero network access, returns per-command results, and leaves the store
//! untouched (`committed == false`).

use base64::Engine;
use iota_types::transaction::TransactionData;
use iota_vm_sdk::{
    Chain, ChainContext, ExecuteOptions, ExecutionMode, InMemoryStore, LocalVm, ObjectId,
    ProtocolVersion, SignatureStatus, StructTag, TypeTag,
};
use move_core_types::annotated_value::MoveValue;

/// Base64-encoded BCS for `0x2::hash::blake2b256([0, 1, 2])` — a pure function
/// whose only dependencies are the framework packages, so it runs against a
/// framework-only store with no extra objects.
const BLAKE2B_TX_B64: &str = "AAABAAQDAAECAQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAgRoYXNoCmJsYWtlMmIyNTYAAQEAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA6AMAAAAAAAAAypo7AAAAAAA=";

fn chain_context() -> ChainContext {
    // Dev-inspect does not need these to match any real network state.
    ChainContext::new(ProtocolVersion::MAX, Chain::Unknown).with_reference_gas_price(1000)
}

#[test]
fn dev_inspect_runs_offline_and_leaves_store_unchanged() {
    let tx_bytes = base64::engine::general_purpose::STANDARD
        .decode(BLAKE2B_TX_B64)
        .expect("base64 decode");
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

/// `decode_value` resolves both a primitive type and a framework struct layout
/// (from the `0x2` package in the store), with no network access.
#[test]
fn decode_value_resolves_primitive_and_framework_struct() {
    let vm = LocalVm::new(chain_context(), InMemoryStore::with_framework()).expect("build LocalVm");

    // Primitive: no package resolution needed.
    let bytes = bcs::to_bytes(&7u64).expect("encode u64");
    let value = vm.decode_value(&bytes, &TypeTag::U64).expect("decode u64");
    assert!(matches!(value, MoveValue::U64(7)), "got {value:?}");

    // Struct: the layout is resolved from the framework package in the store.
    let id = ObjectId::random();
    let bytes = bcs::to_bytes(&id).expect("encode id");
    let tag = TypeTag::from(StructTag::new_id());
    let value = vm.decode_value(&bytes, &tag).expect("decode ID");
    assert!(
        matches!(value, MoveValue::Struct(_)),
        "0x2::object::ID must decode to a struct, got {value:?}"
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
