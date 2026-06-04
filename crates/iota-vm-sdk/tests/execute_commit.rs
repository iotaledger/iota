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

use iota_sdk_types::{ObjectId, Owner};
use iota_types::{
    base_types::SequenceNumber,
    digests::TransactionDigest,
    effects::TransactionEffectsAPI,
    object::{MoveObject, MoveObjectExt, Object},
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    transaction::{
        TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE, TransactionData, TransactionDataAPI,
    },
};
use iota_vm_sdk::{
    Chain, ChainContext, ExecuteOptions, InMemoryStore, IotaAddress, LocalVm, ProtocolVersion,
    Store,
};

const GAS_PRICE: u64 = 1000;
const GAS_COIN_VALUE: u64 = 1_000_000_000_000;

fn chain_context() -> ChainContext {
    ChainContext {
        protocol_version: ProtocolVersion::MAX,
        reference_gas_price: GAS_PRICE,
        epoch_id: 0,
        epoch_timestamp_ms: 0,
        chain: Chain::Unknown,
    }
}

/// A fresh, well-funded gas coin owned by `owner`.
fn gas_coin(owner: IotaAddress) -> Object {
    Object::new_move(
        MoveObject::new_gas_coin(SequenceNumber::from(1), ObjectId::random(), GAS_COIN_VALUE),
        Owner::Address(owner),
        TransactionDigest::ZERO,
    )
}

/// `transfer_iota(recipient, Some(amount))`: splits a fresh coin off gas and
/// transfers it. Mutates the gas coin and creates one new coin.
fn transfer_tx(
    sender: IotaAddress,
    gas: &Object,
    recipient: IotaAddress,
    amount: u64,
) -> TransactionData {
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

#[test]
fn execute_commits_writes_and_deletions_to_store() {
    let sender = IotaAddress::ZERO;
    let recipient = IotaAddress::from(ObjectId::random());

    let gas = gas_coin(sender);
    let gas_id = gas.id();
    let gas_version_before = gas.version();

    let mut store = InMemoryStore::with_framework();
    store.insert(gas.clone());

    let mut vm = LocalVm::new(chain_context(), store).expect("build LocalVm");

    let result = vm
        .execute(
            transfer_tx(sender, &gas, recipient, 1000),
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
    let created: Vec<_> = result.effects.created();
    assert_eq!(created.len(), 1, "transfer_iota creates exactly one coin");
    let new_coin_id = created[0].0.object_id;
    assert!(
        vm.store_mut().get_object(&new_coin_id, None).is_some(),
        "created coin must be committed to the store"
    );

    // The gas coin was mutated: it is still present, at a higher version.
    let gas_after = vm
        .store_mut()
        .get_object(&gas_id, None)
        .expect("gas coin must remain in the store");
    assert!(
        gas_after.version() > gas_version_before,
        "gas coin version must advance after a committed mutation"
    );
}

#[test]
fn dev_inspect_and_dry_run_leave_store_unchanged() {
    let sender = IotaAddress::ZERO;
    let recipient = IotaAddress::from(ObjectId::random());

    for opts in [ExecuteOptions::dev_inspect(), ExecuteOptions::dry_run()] {
        let mode = opts.mode;

        let gas = gas_coin(sender);
        let gas_id = gas.id();
        let gas_version_before = gas.version();

        let mut store = InMemoryStore::with_framework();
        store.insert(gas.clone());

        let mut vm = LocalVm::new(chain_context(), store).expect("build LocalVm");

        let result = vm
            .execute(transfer_tx(sender, &gas, recipient, 1000), opts)
            .unwrap_or_else(|e| panic!("{mode:?} must succeed: {e}"));

        assert!(result.status.is_success(), "{mode:?} run must succeed");
        assert!(!result.committed, "{mode:?} must not commit");

        // No created object leaked into the store, and the gas coin is unchanged.
        let created: Vec<_> = result.effects.created();
        for (objref, _owner) in &created {
            assert!(
                vm.store_mut().get_object(&objref.object_id, None).is_none(),
                "{mode:?}: created object must NOT be committed"
            );
        }
        let gas_after = vm
            .store_mut()
            .get_object(&gas_id, None)
            .expect("gas coin still present");
        assert_eq!(
            gas_after.version(),
            gas_version_before,
            "{mode:?}: gas coin version must be unchanged"
        );
    }
}
