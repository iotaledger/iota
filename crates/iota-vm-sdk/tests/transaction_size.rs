// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The transaction size cap against the public `iota-vm-sdk` API: a
//! transaction above `max_tx_size_bytes` is rejected before it is scanned, as
//! the node rejects it. Self-contained — uses only the built-in framework.

use iota_protocol_config::ProtocolConfig;
use iota_sdk_types::{MoveStruct, ObjectId, Owner, Transaction, TransactionDigest};
use iota_types::{
    error::IotaError,
    object::{MoveStructExt, OBJECT_START_VERSION, Object},
    transaction::TransactionAPI,
    utils::{assert_size_limit, ptb_above_max_tx_size},
};
use iota_vm_sdk::{
    Address, Chain, ChainContext, ExecuteOptions, InMemoryStore, LocalVm, ProtocolVersion, Store,
    VmSdkError,
};

const GAS_PRICE: u64 = 1000;

#[test]
fn dev_inspect_rejects_a_transaction_above_the_size_limit() {
    let sender = Address::ZERO;
    let gas = Object::new_move(
        MoveStruct::new_gas_coin(OBJECT_START_VERSION, ObjectId::random(), 1_000_000_000_000),
        Owner::Address(sender),
        TransactionDigest::ZERO,
    );
    let mut store = InMemoryStore::with_framework();
    store.insert(gas.clone());
    let mut vm = LocalVm::new(
        ChainContext::new(ProtocolVersion::MAX, Chain::Unknown).with_reference_gas_price(GAS_PRICE),
        store,
    )
    .expect("build LocalVm");

    let tx = Transaction::new_programmable(
        sender,
        vec![gas.object_ref()],
        ptb_above_max_tx_size(&ProtocolConfig::get_for_max_version_UNSAFE()),
        10_000_000,
        GAS_PRICE,
    );
    let err = vm
        .execute(tx, ExecuteOptions::dev_inspect())
        .expect_err("a transaction above the size limit must be rejected");
    let VmSdkError::Validation(v) = &err else {
        panic!("got {err:?}");
    };
    let IotaError::UserInput { error } = &v.source else {
        panic!("got {err:?}");
    };
    assert_size_limit(error, "serialized transaction size exceeded maximum");
}
