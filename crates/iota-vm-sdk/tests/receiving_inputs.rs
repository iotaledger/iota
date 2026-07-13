// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Receiving-input handling against the public `iota-vm-sdk` API.
//!
//! Receiving references are the one input kind whose declared version is part
//! of the transaction's runtime semantics: execution resolves the receive at
//! exactly that version, and "already received past this version" is a
//! meaningful outcome. The SDK therefore never rewrites them to the store's
//! versions. `DryRun` applies the node's sign-time checks (declared version
//! and digest must match the store's current object); `DevInspect` skips
//! them, like the node. Self-contained — uses only the built-in framework.

use iota_sdk_types::{ObjectId, ObjectReference, Owner, Version};
use iota_types::{
    digests::TransactionDigest,
    error::{IotaError, UserInputError},
    object::{MoveObject, MoveObjectExt, OBJECT_START_VERSION, Object},
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    transaction::{
        CallArg, TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE, TransactionData,
        TransactionDataAPI,
    },
};
use iota_vm_sdk::{
    Address, Chain, ChainContext, ExecuteOptions, InMemoryStore, LocalVm, ProtocolVersion, Store,
    VmSdkError,
};

const GAS_PRICE: u64 = 1000;
const GAS_COIN_VALUE: u64 = 1_000_000_000_000;

fn chain_context() -> ChainContext {
    ChainContext::new(ProtocolVersion::MAX, Chain::Unknown).with_reference_gas_price(GAS_PRICE)
}

/// A VM whose store holds a funded gas coin for `sender` and a coin at
/// version 5 sent to `parent` (an object id acting as an address), i.e. a
/// receivable object. Returns the VM, the gas coin, and the receivable coin's
/// current reference.
fn vm_with_receivable_coin(
    sender: Address,
    parent: ObjectId,
) -> (LocalVm, Object, ObjectReference) {
    let gas = Object::new_move(
        MoveObject::new_gas_coin(OBJECT_START_VERSION, ObjectId::random(), GAS_COIN_VALUE),
        Owner::Address(sender),
        TransactionDigest::ZERO,
    );
    let receivable = Object::new_move(
        MoveObject::new_gas_coin(Version::from(5), ObjectId::random(), 1),
        Owner::Address(parent.into()),
        TransactionDigest::ZERO,
    );
    let receivable_ref = receivable.object_ref();

    let mut store = InMemoryStore::with_framework();
    store.insert(gas.clone());
    store.insert(receivable);

    let vm = LocalVm::new(chain_context(), store).expect("build LocalVm");
    (vm, gas, receivable_ref)
}

/// A transfer PTB that additionally declares `receiving` as a receiving
/// input. The input stays unused: sign-time checks run on declared inputs
/// regardless of use, which is what these tests exercise.
fn tx_with_receiving_input(
    sender: Address,
    gas: &Object,
    receiving: ObjectReference,
) -> TransactionData {
    let mut b = ProgrammableTransactionBuilder::new();
    b.input(CallArg::Receiving(receiving))
        .expect("add receiving input");
    b.transfer_iota(Address::from(ObjectId::random()), Some(1000));
    TransactionData::new_programmable(
        sender,
        vec![gas.object_ref()],
        b.finish(),
        TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE * GAS_PRICE,
        GAS_PRICE,
    )
}

/// A receiving reference at the store's current version and digest passes the
/// dry-run sign-time checks.
#[test]
fn dry_run_accepts_current_receiving_reference() {
    let sender = Address::ZERO;
    let (mut vm, gas, receivable_ref) = vm_with_receivable_coin(sender, ObjectId::random());

    let result = vm
        .execute(
            tx_with_receiving_input(sender, &gas, receivable_ref),
            ExecuteOptions::dry_run(),
        )
        .expect("dry-run with a current receiving reference must pass the input checks");
    assert!(
        result.status.is_success(),
        "run must succeed, got {:?}",
        result.status
    );
}

/// A receiving reference older than the store's current version is rejected
/// by the dry-run sign-time checks, as the node rejects it at signing: the
/// object was received past that version and can no longer be consumed at it.
#[test]
fn dry_run_rejects_outdated_receiving_version() {
    let sender = Address::ZERO;
    let (mut vm, gas, receivable_ref) = vm_with_receivable_coin(sender, ObjectId::random());

    let stale_ref = ObjectReference::new(
        receivable_ref.object_id,
        Version::from(4),
        receivable_ref.digest,
    );
    let err = vm
        .execute(
            tx_with_receiving_input(sender, &gas, stale_ref),
            ExecuteOptions::dry_run(),
        )
        .expect_err("an outdated receiving version must be rejected");
    assert!(
        matches!(
            &err,
            VmSdkError::Validation(v) if matches!(
                &v.source,
                IotaError::UserInput {
                    error: UserInputError::ObjectVersionUnavailableForConsumption { .. }
                }
            )
        ),
        "got {err:?}"
    );
}

/// A receiving reference with the current version but a wrong digest is
/// rejected by the dry-run sign-time checks, as the node rejects it at
/// signing.
#[test]
fn dry_run_rejects_wrong_receiving_digest() {
    let sender = Address::ZERO;
    let (mut vm, gas, receivable_ref) = vm_with_receivable_coin(sender, ObjectId::random());

    let wrong_digest_ref = ObjectReference::new(
        receivable_ref.object_id,
        receivable_ref.version,
        iota_sdk_types::ObjectDigest::ZERO,
    );
    let err = vm
        .execute(
            tx_with_receiving_input(sender, &gas, wrong_digest_ref),
            ExecuteOptions::dry_run(),
        )
        .expect_err("a wrong receiving digest must be rejected");
    assert!(
        matches!(
            &err,
            VmSdkError::Validation(v) if matches!(
                &v.source,
                IotaError::UserInput {
                    error: UserInputError::InvalidObjectDigest { .. }
                }
            )
        ),
        "got {err:?}"
    );
}

/// Dev-inspect skips the receiving sign-time checks, matching the node: an
/// outdated reference is not rejected up front — a failure would only surface
/// when the receive is executed, which resolves at the declared version.
#[test]
fn dev_inspect_skips_receiving_checks() {
    let sender = Address::ZERO;
    let (mut vm, gas, receivable_ref) = vm_with_receivable_coin(sender, ObjectId::random());

    let stale_ref = ObjectReference::new(
        receivable_ref.object_id,
        Version::from(4),
        receivable_ref.digest,
    );
    let result = vm
        .execute(
            tx_with_receiving_input(sender, &gas, stale_ref),
            ExecuteOptions::dev_inspect(),
        )
        .expect("dev-inspect must not reject an outdated receiving reference up front");
    assert!(
        result.status.is_success(),
        "the receiving input is unused, so the run must succeed, got {:?}",
        result.status
    );
}
