// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Failure-mode coverage for the public `iota-vm-sdk` API: signature-status
//! reporting for standard signature schemes, missing input objects, and
//! unsupported protocol versions.

use iota_sdk_types::{ObjectId, Owner};
use iota_types::{
    base_types::SequenceNumber,
    crypto::{AccountKeyPair, get_key_pair},
    digests::TransactionDigest,
    object::{MoveObject, MoveObjectExt, Object},
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    transaction::{
        TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE, TransactionData, TransactionDataAPI,
    },
    utils::to_sender_signed_transaction,
};
use iota_vm_sdk::{
    Chain, ChainContext, ExecuteOptions, InMemoryStore, IotaAddress, LocalVm, ProtocolVersion,
    SignatureStatus, Store, VmSdkError,
};

const GAS_PRICE: u64 = 1000;
const GAS_COIN_VALUE: u64 = 1_000_000_000_000;

fn chain_context() -> ChainContext {
    ChainContext::new(ProtocolVersion::MAX, GAS_PRICE, 0, 0, Chain::Unknown)
}

fn gas_coin(owner: IotaAddress) -> Object {
    Object::new_move(
        MoveObject::new_gas_coin(SequenceNumber::from(1), ObjectId::random(), GAS_COIN_VALUE),
        Owner::Address(owner),
        TransactionDigest::ZERO,
    )
}

#[test]
fn standard_signature_is_verified_on_success() {
    let (sender, key): (IotaAddress, AccountKeyPair) = get_key_pair();
    let gas = gas_coin(sender);
    let recipient = IotaAddress::from(ObjectId::random());

    let mut b = ProgrammableTransactionBuilder::new();
    b.transfer_iota(recipient, Some(1000));
    let tx = TransactionData::new_programmable(
        sender,
        vec![gas.object_ref()],
        b.finish(),
        TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE * GAS_PRICE,
        GAS_PRICE,
    );
    let signed = to_sender_signed_transaction(tx, &key).into_data();

    let mut store = InMemoryStore::with_framework();
    store.insert(gas);
    let mut vm = LocalVm::new(chain_context(), store).expect("build LocalVm");

    let result = vm
        .execute_signed(signed, ExecuteOptions::dry_run())
        .expect("signed dry-run must succeed");
    assert!(result.status.is_success(), "transfer must succeed");
    assert!(
        matches!(result.signature_status, SignatureStatus::Verified),
        "valid ed25519 signature must report Verified, got {:?}",
        result.signature_status
    );
}

/// A failure in the transaction body must not be misreported as a signature
/// failure: the ed25519 signature verified fine.
#[test]
fn standard_signature_stays_verified_when_body_aborts() {
    let (sender, key): (IotaAddress, AccountKeyPair) = get_key_pair();
    let gas = gas_coin(sender);
    let recipient = IotaAddress::from(ObjectId::random());

    // Try to split off more than the gas coin holds: the body aborts.
    let mut b = ProgrammableTransactionBuilder::new();
    b.transfer_iota(recipient, Some(GAS_COIN_VALUE * 2));
    let tx = TransactionData::new_programmable(
        sender,
        vec![gas.object_ref()],
        b.finish(),
        TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE * GAS_PRICE,
        GAS_PRICE,
    );
    let signed = to_sender_signed_transaction(tx, &key).into_data();

    let mut store = InMemoryStore::with_framework();
    store.insert(gas);
    let mut vm = LocalVm::new(chain_context(), store).expect("build LocalVm");

    let result = vm
        .execute_signed(signed, ExecuteOptions::dry_run())
        .expect("the run itself returns Ok; the abort is carried in the status");
    assert!(
        !result.status.is_success(),
        "overspending transfer must abort"
    );
    assert!(
        matches!(result.signature_status, SignatureStatus::Verified),
        "a body abort must not be reported as a signature failure, got {:?}",
        result.signature_status
    );
}

#[test]
fn missing_input_object_is_reported_with_its_id() {
    let gas = gas_coin(IotaAddress::ZERO);
    let phantom_id = gas.id();
    let recipient = IotaAddress::from(ObjectId::random());
    let mut b = ProgrammableTransactionBuilder::new();
    b.transfer_iota(recipient, Some(1000));
    let tx = TransactionData::new_programmable(
        IotaAddress::ZERO,
        vec![gas.object_ref()],
        b.finish(),
        TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE * GAS_PRICE,
        GAS_PRICE,
    );

    // The store never saw the gas coin.
    let store = InMemoryStore::with_framework();
    let mut vm = LocalVm::new(chain_context(), store).expect("build LocalVm");

    let Err(err) = vm.execute(tx, ExecuteOptions::dry_run()) else {
        panic!("a missing gas object must fail preparation");
    };
    match err {
        VmSdkError::MissingObject { id, .. } => assert_eq!(id, phantom_id),
        other => panic!("expected MissingObject, got {other:?}"),
    }
}

#[test]
fn unsupported_protocol_version_is_an_error_not_a_panic() {
    let ctx = ChainContext::new(
        ProtocolVersion::new(u64::MAX),
        GAS_PRICE,
        0,
        0,
        Chain::Unknown,
    );
    let err = LocalVm::new(ctx, InMemoryStore::new())
        .map(|_| ())
        .expect_err("a future protocol version must be rejected");
    assert!(
        matches!(
            err,
            VmSdkError::UnsupportedProtocolVersion { version } if version.as_u64() == u64::MAX
        ),
        "expected UnsupportedProtocolVersion, got {err:?}"
    );
}
