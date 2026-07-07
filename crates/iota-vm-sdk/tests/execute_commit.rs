// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! `ExecutionMode` store-commit semantics against the public `iota-vm-sdk` API.
//!
//! Asserts the store-commit contract for the three modes:
//! - `Execute` applies effects (writes *and* deletions) back to the store and
//!   sets `committed == true` on success — a follow-up call sees the
//!   post-state.
//! - `DevInspect` / `DryRun` leave the store untouched and set `committed ==
//!   false`.
//!
//! Self-contained: seeds one real gas coin and runs a `transfer_iota` PTB
//! (splits a fresh coin off gas and transfers it), which both mutates the gas
//! coin and creates a new coin — using only the built-in framework, no Move
//! compiler.

use iota_sdk_types::{
    ObjectId, Owner,
    transaction::{GenesisTransaction, TransactionKind},
};
use iota_types::{
    digests::TransactionDigest,
    effects::TransactionEffectsAPI,
    object::{MoveObject, MoveObjectExt, OBJECT_START_VERSION, Object},
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    transaction::{
        TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE, TransactionData, TransactionDataAPI,
    },
};
use iota_vm_sdk::{
    Address, Chain, ChainContext, ExecuteOptions, InMemoryStore, LocalVm, ProtocolVersion, Store,
    TransactionDenyConfigBuilder, VmSdkError,
};

const GAS_PRICE: u64 = 1000;
const GAS_COIN_VALUE: u64 = 1_000_000_000_000;
const TRANSFER_AMOUNT: u64 = 1000;

fn chain_context() -> ChainContext {
    ChainContext::new(ProtocolVersion::MAX, Chain::Unknown).with_reference_gas_price(GAS_PRICE)
}

/// A fresh, well-funded gas coin owned by `owner`.
fn gas_coin(owner: Address) -> Object {
    Object::new_move(
        MoveObject::new_gas_coin(OBJECT_START_VERSION, ObjectId::random(), GAS_COIN_VALUE),
        Owner::Address(owner),
        TransactionDigest::ZERO,
    )
}

/// `transfer_iota(recipient, Some(amount))`: splits a fresh coin off gas and
/// transfers it. Mutates the gas coin and creates one new coin.
fn transfer_tx(sender: Address, gas: &Object, recipient: Address, amount: u64) -> TransactionData {
    let mut b = ProgrammableTransactionBuilder::new();
    b.transfer_iota(recipient, Some(amount));
    TransactionData::new_programmable(
        sender,
        vec![gas.object_ref()],
        b.finish(),
        TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE * GAS_PRICE,
        GAS_PRICE,
    )
}

/// The same transfer PTB without any gas payment.
fn gasless_transfer_tx(sender: Address, recipient: Address, amount: u64) -> TransactionData {
    let mut b = ProgrammableTransactionBuilder::new();
    b.transfer_iota(recipient, Some(amount));
    TransactionData::new_programmable(
        sender,
        vec![],
        b.finish(),
        TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE * GAS_PRICE,
        GAS_PRICE,
    )
}

#[test]
fn execute_commits_writes_to_store() {
    let sender = Address::ZERO;
    let recipient = Address::from(ObjectId::random());

    let gas = gas_coin(sender);
    let gas_id = gas.id();
    let gas_version_before = gas.version();

    let mut store = InMemoryStore::with_framework();
    store.insert(gas.clone());

    let mut vm = LocalVm::new(chain_context(), store).expect("build LocalVm");

    let result = vm
        .execute(
            transfer_tx(sender, &gas, recipient, TRANSFER_AMOUNT),
            ExecuteOptions::execute(),
        )
        .expect("execute must succeed");

    assert!(
        result.status.is_success(),
        "transfer must succeed, got {:?}",
        result.status
    );
    assert!(
        result.committed,
        "successful Execute must set committed = true"
    );

    // A new coin was created and is now committed to the store.
    let created = result.effects.created();
    assert_eq!(created.len(), 1, "transfer_iota creates exactly one coin");
    let new_coin_id = created[0].0.object_id;
    assert!(
        vm.store()
            .get_object(&new_coin_id, None)
            .expect("store lookup")
            .is_some(),
        "created coin must be committed to the store"
    );

    // The gas coin was mutated: it is still present, at a higher version.
    let gas_after = vm
        .store()
        .get_object(&gas_id, None)
        .expect("store lookup")
        .expect("gas coin must remain in the store");
    assert!(
        gas_after.version() > gas_version_before,
        "gas coin version must advance after a committed mutation"
    );
}

/// `Execute` mode must also commit deletions: `pay` merges the second input
/// coin into the first, consuming (deleting) it — it must be removed from the
/// store.
#[test]
fn execute_commits_deletions_to_store() {
    let sender = Address::ZERO;
    let recipient = Address::from(ObjectId::random());

    let gas = gas_coin(sender);
    // Two owned coins; `pay` merges the second into the first, deleting it.
    let primary = gas_coin(sender);
    let merged = gas_coin(sender);
    let merged_id = merged.id();

    let mut store = InMemoryStore::with_framework();
    store.insert(gas.clone());
    store.insert(primary.clone());
    store.insert(merged.clone());

    let mut vm = LocalVm::new(chain_context(), store).expect("build LocalVm");

    let mut b = ProgrammableTransactionBuilder::new();
    b.pay(
        vec![primary.object_ref(), merged.object_ref()],
        vec![recipient],
        vec![TRANSFER_AMOUNT],
    )
    .expect("build pay PTB");
    let tx = TransactionData::new_programmable(
        sender,
        vec![gas.object_ref()],
        b.finish(),
        TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE * GAS_PRICE,
        GAS_PRICE,
    );

    let result = vm
        .execute(tx, ExecuteOptions::execute())
        .expect("execute must succeed");
    assert!(
        result.status.is_success(),
        "pay must succeed, got {:?}",
        result.status
    );
    assert!(
        result.committed,
        "successful Execute must set committed = true"
    );

    // The merged-in coin is reported deleted and is gone from the store.
    assert!(
        result
            .effects
            .deleted()
            .iter()
            .any(|o| o.object_id == merged_id),
        "merged coin must appear in effects.deleted()"
    );
    assert!(
        vm.store()
            .get_object(&merged_id, None)
            .expect("store lookup")
            .is_none(),
        "deleted coin must be removed from the store"
    );
}

#[test]
fn dev_inspect_and_dry_run_leave_store_unchanged() {
    let sender = Address::ZERO;
    let recipient = Address::from(ObjectId::random());

    for opts in [ExecuteOptions::dev_inspect(), ExecuteOptions::dry_run()] {
        let mode = opts.mode;

        let gas = gas_coin(sender);
        let gas_id = gas.id();
        let gas_version_before = gas.version();

        let mut store = InMemoryStore::with_framework();
        store.insert(gas.clone());

        let mut vm = LocalVm::new(chain_context(), store).expect("build LocalVm");

        let result = vm
            .execute(transfer_tx(sender, &gas, recipient, TRANSFER_AMOUNT), opts)
            .unwrap_or_else(|e| panic!("{mode:?} must succeed: {e}"));

        assert!(result.status.is_success(), "{mode:?} run must succeed");
        assert!(!result.committed, "{mode:?} must not commit");

        // The effects still report the created coin; it must not leak into the
        // store, and the gas coin is unchanged.
        let created = result.effects.created();
        assert_eq!(
            created.len(),
            1,
            "{mode:?}: effects must report the created coin"
        );
        assert!(
            vm.store()
                .get_object(&created[0].0.object_id, None)
                .expect("store lookup")
                .is_none(),
            "{mode:?}: created object must NOT be committed"
        );
        let gas_after = vm
            .store()
            .get_object(&gas_id, None)
            .expect("store lookup")
            .expect("gas coin still present");
        assert_eq!(
            gas_after.version(),
            gas_version_before,
            "{mode:?}: gas coin version must be unchanged"
        );
    }
}

/// `Execute` requires a real gas payment: a gasless transaction is rejected
/// instead of being funded with the mock simulation coin and committed.
#[test]
fn execute_rejects_gasless_transaction() {
    let sender = Address::ZERO;
    let recipient = Address::from(ObjectId::random());

    let mut vm =
        LocalVm::new(chain_context(), InMemoryStore::with_framework()).expect("build LocalVm");

    let err = vm
        .execute(
            gasless_transfer_tx(sender, recipient, TRANSFER_AMOUNT),
            ExecuteOptions::execute(),
        )
        .expect_err("gasless Execute must be rejected");
    assert!(matches!(err, VmSdkError::Validation(_)), "got {err:?}");
}

/// Gasless dev-inspect and dry-run runs are funded with the one-shot mock
/// coin, which is never persisted.
#[test]
fn dev_inspect_and_dry_run_fund_gasless_transactions_with_mock_coin() {
    let sender = Address::ZERO;
    let recipient = Address::from(ObjectId::random());

    for opts in [ExecuteOptions::dev_inspect(), ExecuteOptions::dry_run()] {
        let mode = opts.mode;
        let mut vm =
            LocalVm::new(chain_context(), InMemoryStore::with_framework()).expect("build LocalVm");

        let result = vm
            .execute(
                gasless_transfer_tx(sender, recipient, TRANSFER_AMOUNT),
                opts,
            )
            .unwrap_or_else(|e| panic!("{mode:?} gasless run must succeed: {e}"));

        assert!(
            result.status.is_success(),
            "{mode:?} gasless run must succeed, got {:?}",
            result.status
        );
        assert!(!result.committed, "{mode:?} must not commit");
        let mock_gas_id = result.mock_gas_id.expect("mock gas coin must be minted");
        assert!(
            vm.store()
                .get_object(&mock_gas_id, None)
                .expect("store lookup")
                .is_none(),
            "{mode:?}: mock gas coin must not be persisted"
        );
    }
}

/// Dev-inspect meters at `max_tx_gas` even when a real gas coin with a lower
/// declared budget is supplied, matching the node's dev-inspect entry point.
#[test]
fn dev_inspect_with_real_gas_coin_ignores_declared_budget() {
    let sender = Address::ZERO;
    let recipient = Address::from(ObjectId::random());

    let gas = gas_coin(sender);
    let mut store = InMemoryStore::with_framework();
    store.insert(gas.clone());
    let mut vm = LocalVm::new(chain_context(), store).expect("build LocalVm");

    let mut tx = transfer_tx(sender, &gas, recipient, TRANSFER_AMOUNT);
    tx.gas_data_mut().budget = 0;

    let result = vm
        .execute(tx, ExecuteOptions::dev_inspect())
        .expect("dev-inspect must not error");
    assert!(
        result.status.is_success(),
        "zero-budget dev-inspect with a real gas coin must succeed, got {:?}",
        result.status
    );
    assert!(
        result.mock_gas_id.is_none(),
        "a supplied gas coin must be used as-is"
    );
}

/// System transactions are rejected in every mode.
#[test]
fn system_transactions_are_rejected() {
    let sender = Address::ZERO;

    for opts in [
        ExecuteOptions::dev_inspect(),
        ExecuteOptions::dry_run(),
        ExecuteOptions::execute(),
    ] {
        let mode = opts.mode;
        let mut vm =
            LocalVm::new(chain_context(), InMemoryStore::with_framework()).expect("build LocalVm");

        let tx = TransactionData::new_with_gas_coins(
            TransactionKind::Genesis(GenesisTransaction {
                objects: vec![],
                events: vec![],
            }),
            sender,
            vec![],
            TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE * GAS_PRICE,
            GAS_PRICE,
        );

        let err = vm
            .execute(tx, opts)
            .expect_err("system transaction must be rejected");
        assert!(
            matches!(err, VmSdkError::Validation(_)),
            "{mode:?}: got {err:?}"
        );
    }
}

/// The deny-list configuration on `ExecuteOptions` is enforced.
#[test]
fn deny_config_rejects_denied_sender() {
    let sender = Address::ZERO;
    let recipient = Address::from(ObjectId::random());

    let gas = gas_coin(sender);
    let mut store = InMemoryStore::with_framework();
    store.insert(gas.clone());
    let mut vm = LocalVm::new(chain_context(), store).expect("build LocalVm");

    let deny_config = TransactionDenyConfigBuilder::new()
        .add_denied_address(sender)
        .build();
    let err = vm
        .execute(
            transfer_tx(sender, &gas, recipient, TRANSFER_AMOUNT),
            ExecuteOptions::dry_run().with_deny_config(deny_config),
        )
        .expect_err("a denied sender must be rejected");
    assert!(matches!(err, VmSdkError::Validation(_)), "got {err:?}");
}
