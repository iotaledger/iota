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

use iota_sdk_types::{
    MoveStruct, ObjectId, ObjectReference, Owner, ProgrammableTransaction, Transaction,
    TransactionDigest, Version,
};
use iota_types::{
    error::{IotaError, UserInputError},
    object::{MoveStructExt, OBJECT_START_VERSION, Object},
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    transaction::{CallArg, TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE, TransactionAPI},
};
use iota_vm_sdk::{
    Address, Chain, ChainContext, ExecuteOptions, ExecutionResult, InMemoryStore, LocalVm,
    ProtocolVersion, Store, VmSdkError,
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
        MoveStruct::new_gas_coin(OBJECT_START_VERSION, ObjectId::random(), GAS_COIN_VALUE),
        Owner::Address(sender),
        TransactionDigest::ZERO,
    );
    let receivable = Object::new_move(
        MoveStruct::new_gas_coin(Version::from(5), ObjectId::random(), 1),
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
) -> Transaction {
    let mut b = ProgrammableTransactionBuilder::new();
    b.input(CallArg::Receiving(receiving))
        .expect("add receiving input");
    b.transfer_iota(Address::from(ObjectId::random()), Some(1000));
    Transaction::new_programmable(
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

/// Runs `tx` under both execution modes and returns what each produced.
///
/// Only whether a receiving reference is *current* is relaxed for a dev
/// inspect. What the object is, and that no reference is named twice, is
/// checked either way, so these assert over both modes.
fn execute_under_both_modes(
    vm: &mut LocalVm,
    tx: Transaction,
) -> Vec<(&'static str, Result<ExecutionResult, VmSdkError>)> {
    vec![
        ("dry run", vm.execute(tx.clone(), ExecuteOptions::dry_run())),
        ("dev inspect", vm.execute(tx, ExecuteOptions::dev_inspect())),
    ]
}

/// A transfer PTB with `extra` appended as declared-but-unused inputs.
///
/// [`ProgrammableTransactionBuilder`] deduplicates object inputs and refuses to
/// name one object under two argument kinds, so a malformed set of references
/// cannot be built through it. A client sends the transaction as bytes and is
/// under no such constraint, which is what the input checks answer for.
fn tx_with_extra_inputs(sender: Address, gas: &Object, extra: Vec<CallArg>) -> Transaction {
    let mut b = ProgrammableTransactionBuilder::new();
    b.transfer_iota(Address::from(ObjectId::random()), Some(1000));
    let ProgrammableTransaction {
        mut inputs,
        commands,
    } = b.finish();
    inputs.extend(extra);

    Transaction::new_programmable(
        sender,
        vec![gas.object_ref()],
        ProgrammableTransaction { inputs, commands },
        TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE * GAS_PRICE,
        GAS_PRICE,
    )
}

fn assert_user_input_error(
    label: &str,
    result: Result<ExecutionResult, VmSdkError>,
    expected: impl Fn(&UserInputError) -> bool,
) {
    let Err(VmSdkError::Validation(validation)) = &result else {
        panic!("{label} must reject the transaction, got {result:?}");
    };
    let IotaError::UserInput { error } = &validation.source else {
        panic!("{label} must fail the input checks, got {validation:?}");
    };
    assert!(expected(error), "unexpected error for {label}: {error:?}");
}

/// The same object named by two receiving references is rejected under both
/// modes.
///
/// `CallArg::Receiving` is not part of `input_objects()`, so it escapes that
/// function's duplicate rejection; this check is the only one there is. Without
/// it both tickets reach the object runtime, which treats receiving one object
/// twice as impossible.
#[test]
fn both_modes_reject_a_duplicate_receiving_reference() {
    let sender = Address::ZERO;
    let (mut vm, gas, receivable_ref) = vm_with_receivable_coin(sender, ObjectId::random());

    let tx = tx_with_extra_inputs(
        sender,
        &gas,
        vec![
            CallArg::Receiving(receivable_ref),
            CallArg::Receiving(receivable_ref),
        ],
    );

    for (label, result) in execute_under_both_modes(&mut vm, tx) {
        assert_user_input_error(label, result, |error| {
            matches!(error, UserInputError::DuplicateObjectRefInput)
        });
    }
}

/// An object named both as an owned input and as a receiving reference is
/// rejected under both modes.
#[test]
fn both_modes_reject_a_receiving_reference_that_is_also_an_input() {
    let sender = Address::ZERO;
    let gas = Object::new_move(
        MoveStruct::new_gas_coin(OBJECT_START_VERSION, ObjectId::random(), GAS_COIN_VALUE),
        Owner::Address(sender),
        TransactionDigest::ZERO,
    );
    // Owned by the sender, so the owned-input checks pass and the collision
    // between the two references is what rejects the transaction.
    let owned = Object::new_move(
        MoveStruct::new_gas_coin(OBJECT_START_VERSION, ObjectId::random(), 1),
        Owner::Address(sender),
        TransactionDigest::ZERO,
    );
    let owned_ref = owned.object_ref();

    let mut store = InMemoryStore::with_framework();
    store.insert(gas.clone());
    store.insert(owned);
    let mut vm = LocalVm::new(chain_context(), store).expect("build LocalVm");

    let tx = tx_with_extra_inputs(
        sender,
        &gas,
        vec![
            CallArg::ImmutableOrOwned(owned_ref),
            CallArg::Receiving(owned_ref),
        ],
    );

    for (label, result) in execute_under_both_modes(&mut vm, tx) {
        assert_user_input_error(label, result, |error| {
            matches!(error, UserInputError::DuplicateObjectRefInput)
        });
    }
}

/// A package named as a receiving reference is rejected under both modes: a
/// package is not a value that can be received.
#[test]
fn both_modes_reject_receiving_a_package() {
    let sender = Address::ZERO;
    let (mut vm, gas, _) = vm_with_receivable_coin(sender, ObjectId::random());
    let package = vm
        .store()
        .get_object(&ObjectId::FRAMEWORK, None)
        .expect("store lookup")
        .expect("framework package");

    let tx = tx_with_receiving_input(sender, &gas, package.object_ref());

    for (label, result) in execute_under_both_modes(&mut vm, tx) {
        assert_user_input_error(label, result, |error| {
            matches!(
                error,
                UserInputError::MovePackageAsObject { object_id } if *object_id == ObjectId::FRAMEWORK
            )
        });
    }
}
