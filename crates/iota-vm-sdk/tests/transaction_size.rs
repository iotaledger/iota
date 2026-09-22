// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The transaction size cap against the public `iota-vm-sdk` API: a
//! transaction above `max_tx_size_bytes` is rejected before it is scanned, as
//! the node rejects it. Self-contained — uses only the built-in framework.

use iota_protocol_config::ProtocolConfig;
use iota_sdk_types::{MoveStruct, ObjectId, Owner, Transaction, TransactionDigest};
use iota_types::{
    crypto::{AccountPrivateKey, get_key_pair},
    error::IotaError,
    object::{MoveStructExt, OBJECT_START_VERSION, Object},
    transaction::TransactionAPI,
    utils::{assert_size_limit, ptb_above_max_tx_size, to_sender_signed_transaction},
};
use iota_vm_sdk::{
    Address, Chain, ChainContext, ExecuteOptions, InMemoryStore, LocalVm, ProtocolVersion, Store,
    VmSdkError,
};

const GAS_PRICE: u64 = 1000;

/// An in-memory store holding one gas coin, and a transaction above
/// `max_tx_size_bytes` that spends it.
fn oversized_transaction(sender: Address) -> (InMemoryStore, Transaction) {
    let gas = Object::new_move(
        MoveStruct::new_gas_coin(OBJECT_START_VERSION, ObjectId::random(), 1_000_000_000_000),
        Owner::Address(sender),
        TransactionDigest::ZERO,
    );
    let mut store = InMemoryStore::with_framework();
    store.insert(gas.clone());
    let tx = Transaction::new_programmable(
        sender,
        vec![gas.object_ref()],
        ptb_above_max_tx_size(&ProtocolConfig::get_for_max_version_UNSAFE()),
        10_000_000,
        GAS_PRICE,
    );
    (store, tx)
}

fn build_vm(store: InMemoryStore) -> LocalVm {
    LocalVm::new(
        ChainContext::new(ProtocolVersion::MAX, Chain::Unknown).with_reference_gas_price(GAS_PRICE),
        store,
    )
    .expect("build LocalVm")
}

fn assert_above_the_size_limit(err: VmSdkError) {
    let VmSdkError::Validation(v) = &err else {
        panic!("got {err:?}");
    };
    let IotaError::UserInput { error } = &v.source else {
        panic!("got {err:?}");
    };
    assert_size_limit(error, "serialized transaction size exceeded maximum");
}

/// Every mode, because the cap runs before the mode is read.
fn every_mode() -> [ExecuteOptions; 3] {
    [
        ExecuteOptions::dev_inspect(),
        ExecuteOptions::dry_run(),
        ExecuteOptions::execute(),
    ]
}

#[test]
fn a_transaction_above_the_size_limit_is_rejected_in_every_mode() {
    for opts in every_mode() {
        let (store, tx) = oversized_transaction(Address::ZERO);
        let err = build_vm(store)
            .execute(tx, opts)
            .expect_err("a transaction above the size limit must be rejected");
        assert_above_the_size_limit(err);
    }
}

#[test]
fn a_signed_transaction_above_the_size_limit_is_rejected_in_every_mode() {
    for opts in every_mode() {
        let (sender, key): (Address, AccountPrivateKey) = get_key_pair();
        let (store, tx) = oversized_transaction(sender);
        // The signature has to be valid: on this path the size cap runs after
        // signature verification.
        let signed = to_sender_signed_transaction(tx, &key).into_data();
        let err = build_vm(store)
            .execute_signed(signed, opts)
            .expect_err("a transaction above the size limit must be rejected");
        assert_above_the_size_limit(err);
    }
}
