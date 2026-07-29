// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The transaction kinds built by this module have to stay identical to the
//! ones the JSON-RPC `iota_transaction_builder::TransactionBuilder` produces.
//! Its `*_tx_kind` helpers all delegate to [`ProgrammableTransactionBuilder`]
//! once the object references are resolved, so these tests rebuild the expected
//! programmable transaction with it and compare.
//!
//! Object inputs resolve against [`TestClient`], which fabricates an
//! address-owned coin per id, so the expected references are read back from
//! that same client.

use iota_sdk_transaction_builder::TestClient;
use iota_sdk_types::TransactionKind;
use iota_types::programmable_transaction_builder::ProgrammableTransactionBuilder;

use super::*;

fn address(byte: u8) -> Address {
    Address::new([byte; Address::LENGTH])
}

fn object_id(byte: u8) -> ObjectId {
    ObjectId::new([byte; ObjectId::LENGTH])
}

fn builder() -> TransactionBuilder<TestClient> {
    TransactionBuilder::new(address(1)).with_client(TestClient)
}

/// The reference [`TestClient`] resolves `object_id` to.
async fn object_ref(object_id: ObjectId) -> ObjectReference {
    let object = TestClient
        .object(object_id, None)
        .await
        .expect("the test client fabricates every object")
        .expect("the test client never reports an object as missing");
    ObjectReference::new(object_id, object.version(), object.digest())
}

#[tokio::test]
async fn pay_matches_json_rpc_builder() {
    let coins = [object_id(3), object_id(4), object_id(5)];
    // A repeated recipient has to end up in a single transfer command, and a
    // repeated amount in a single pure input.
    let recipients = [address(6), address(7), address(6)];
    let amounts = [100, 200, 100];

    let mut coin_refs = Vec::new();
    for coin in coins {
        coin_refs.push(object_ref(coin).await);
    }
    let mut expected = ProgrammableTransactionBuilder::new();
    expected
        .pay(coin_refs, recipients.to_vec(), amounts.to_vec())
        .unwrap();

    let mut builder = builder();
    builder.pay(&coins, &recipients, &amounts).unwrap();

    assert_eq!(
        builder.finish_kind().await.unwrap(),
        TransactionKind::new_programmable(expected.finish()),
    );
}

#[tokio::test]
async fn pay_with_a_single_coin_matches_json_rpc_builder() {
    let coins = [object_id(3)];
    let recipients = [address(6)];
    let amounts = [100];

    let mut expected = ProgrammableTransactionBuilder::new();
    expected
        .pay(
            vec![object_ref(coins[0]).await],
            recipients.to_vec(),
            amounts.to_vec(),
        )
        .unwrap();

    let mut builder = builder();
    builder.pay(&coins, &recipients, &amounts).unwrap();

    assert_eq!(
        builder.finish_kind().await.unwrap(),
        TransactionKind::new_programmable(expected.finish()),
    );
}

#[tokio::test]
async fn pay_iota_matches_json_rpc_builder() {
    let recipients = [address(6), address(7), address(6)];
    let amounts = [100, 200, 100];

    let mut expected = ProgrammableTransactionBuilder::new();
    expected
        .pay_iota(recipients.to_vec(), amounts.to_vec())
        .unwrap();

    let mut builder = builder();
    builder.pay_iota(&recipients, &amounts).unwrap();

    assert_eq!(
        builder.finish_kind().await.unwrap(),
        TransactionKind::new_programmable(expected.finish()),
    );
}

#[tokio::test]
async fn pay_all_iota_matches_json_rpc_builder() {
    let recipient = address(6);

    let mut expected = ProgrammableTransactionBuilder::new();
    expected.pay_all_iota(recipient);

    let mut builder = builder();
    builder.pay_all_iota(recipient);

    assert_eq!(
        builder.finish_kind().await.unwrap(),
        TransactionKind::new_programmable(expected.finish()),
    );
}

/// `finish_kind` must not pull gas coins into the transaction: the client
/// commands select the gas payment themselves.
#[tokio::test]
async fn finish_kind_leaves_the_gas_payment_empty() {
    let mut builder = builder();
    builder.pay_iota(&[address(6)], &[100]).unwrap();

    let TransactionKind::Programmable(ptb) = builder.finish_kind().await.unwrap() else {
        panic!("expected a programmable transaction");
    };
    // Only the amount and the recipient, no gas coin the builder picked itself.
    assert_eq!(ptb.inputs.len(), 2);
}

#[test]
fn pay_rejects_mismatched_recipients_and_amounts() {
    assert!(builder().pay_iota(&[address(6)], &[100, 200]).is_err());
}

#[test]
fn pay_rejects_an_empty_coin_list() {
    assert!(builder().pay(&[], &[address(6)], &[100]).is_err());
}
