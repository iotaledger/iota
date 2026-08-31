// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Tests for the node-internal [`TransactionBuilderLedgerClient`]
//! implementation: the SDK `TransactionBuilder` resolves objects, gas, and
//! protocol parameters directly from a fullnode's local state, without going
//! through a public API.

use iota_macros::sim_test;
use iota_sdk_transaction_builder::{TransactionBuilder, TransactionBuilderLedgerClient};
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
        .with(|node| node.transaction_builder_ledger_client());

    // Reads against local state.
    let reference_gas_price = client
        .reference_gas_price(None)
        .await
        .unwrap()
        .expect("current epoch always has a reference gas price");
    assert!(reference_gas_price > 0);

    let protocol_config = client.protocol_config().await.unwrap();
    let max_tx_gas: u64 = protocol_config.attributes["max_tx_gas"]
        .parse()
        .expect("attribute values are stringified numbers");
    assert!(max_tx_gas > 0);
    assert!(
        protocol_config
            .attributes
            .contains_key("max_gas_payment_objects")
    );

    // A zero limit falls back to the default page size instead of clamping
    // to one, matching the gRPC server.
    let full_page = client
        .objects(Some(StructTag::new_gas_coin()), sender, None, Some(0))
        .await
        .unwrap();
    assert!(full_page.data.len() > 1);
    assert!(full_page.next_cursor.is_none());

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
            Some(cursor.clone()),
            Some(1),
        )
        .await
        .unwrap();
    assert_eq!(second_page.data.len(), 1);
    assert_ne!(first_page.data[0].id(), second_page.data[0].id());

    // The cursor is bound to the request that produced it: replaying it
    // with a different owner or type filter is rejected instead of silently
    // resuming from a meaningless position.
    let mismatched_owner = client
        .objects(
            Some(StructTag::new_gas_coin()),
            recipient,
            Some(cursor.clone()),
            Some(1),
        )
        .await;
    assert!(
        matches!(
            mismatched_owner,
            Err(iota_node_transaction_builder::Error::CursorMismatch)
        ),
        "a cursor from another owner's listing must be rejected: {mismatched_owner:?}",
    );
    let mismatched_filter = client.objects(None, sender, Some(cursor), Some(1)).await;
    assert!(
        matches!(
            mismatched_filter,
            Err(iota_node_transaction_builder::Error::CursorMismatch)
        ),
        "a cursor from a differently-filtered listing must be rejected: {mismatched_filter:?}",
    );

    // Object lookup by id.
    let coin = client
        .object(first_page.data[0].id(), None)
        .await
        .unwrap()
        .expect("listed object exists");
    assert_eq!(coin.id(), first_page.data[0].id());

    // The client is ledger-only, so `finish()` (which estimates the budget)
    // does not exist for it; the budget is passed explicitly instead, and the
    // builder resolves gas coins and the gas price through the client.
    let mut builder = TransactionBuilder::new(sender).with_client(client);
    builder.send_iota(recipient, 1_000_000u64);
    let transaction = builder.finish_with_budget(50_000_000).await.unwrap();
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
