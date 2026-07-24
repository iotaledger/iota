// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Tests for the node-internal [`TransactionBuilderResolveClient`]
//! implementation: the SDK `TransactionBuilder` resolves objects, gas, and
//! protocol parameters directly from a fullnode's local state, without going
//! through a public API.

use iota_macros::sim_test;
use iota_sdk_transaction_builder::{
    TransactionBuilder, TransactionBuilderResolveClient, error::Error,
};
use iota_sdk_types::{StructTag, Transaction};
use test_cluster::TestClusterBuilder;

#[sim_test]
async fn transaction_builder_via_node_internal_client() {
    let test_cluster = TestClusterBuilder::new()
        .disable_fullnode_pruning()
        .build()
        .await;
    let sender = test_cluster.get_address_0();
    let recipient = test_cluster.get_address_1();

    let client = test_cluster
        .fullnode_handle
        .iota_node
        .with(|node| node.transaction_builder_resolve_client());

    // Reads against local state.
    let reference_gas_price = client
        .reference_gas_price(None)
        .await
        .unwrap()
        .expect("current epoch always has a reference gas price");
    assert!(reference_gas_price > 0);

    let protocol_config = client.protocol_config().await.unwrap();
    assert!(protocol_config.attributes.contains_key("max_tx_gas"));
    assert!(
        protocol_config
            .attributes
            .contains_key("max_gas_payment_objects")
    );

    // Owned-object listing with cursor pagination through the gRPC indexes.
    let first_page = client
        .objects(Some(StructTag::new_gas_coin()), sender, None, Some(1))
        .await
        .unwrap();
    assert_eq!(first_page.data.len(), 1);
    let cursor = first_page
        .next_cursor
        .clone()
        .expect("test accounts start with multiple gas coins");
    let second_page = client
        .objects(
            Some(StructTag::new_gas_coin()),
            sender,
            Some(cursor),
            Some(1),
        )
        .await
        .unwrap();
    assert_eq!(second_page.data.len(), 1);
    assert_ne!(first_page.data[0].id(), second_page.data[0].id());

    // Object lookup by id.
    let coin = client
        .object(first_page.data[0].id(), None)
        .await
        .unwrap()
        .expect("listed object exists");
    assert_eq!(coin.id(), first_page.data[0].id());

    // The client is read-only and cannot estimate the gas budget, so building
    // without an explicit budget must fail.
    let mut builder = TransactionBuilder::new(sender).with_client(client.clone());
    builder.send_iota(recipient, 1_000_000u64);
    assert!(matches!(
        builder.finish().await,
        Err(Error::MissingGasBudget)
    ));

    // With an explicit budget, the builder resolves gas coins and the gas
    // price through the client and produces a valid transaction.
    let mut builder = TransactionBuilder::new(sender).with_client(client.clone());
    builder.send_iota(recipient, 1_000_000u64);
    builder.gas_budget(50_000_000);
    let transaction = builder.finish().await.unwrap();
    let Transaction::V1(transaction_v1) = &transaction else {
        panic!("the builder produces V1 transactions");
    };
    assert_eq!(transaction_v1.gas_payment.budget, 50_000_000);
    assert_eq!(transaction_v1.gas_payment.price, reference_gas_price);
    assert!(!transaction_v1.gas_payment.objects.is_empty());

    // The built transaction is valid: signing and executing it through the
    // wallet succeeds.
    let signed = test_cluster.wallet.sign_transaction(&transaction);
    test_cluster
        .wallet
        .execute_transaction_must_succeed(signed)
        .await;
}
