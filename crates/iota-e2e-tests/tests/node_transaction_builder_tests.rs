// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Tests for the node-internal [`TransactionBuilderClient`] implementation:
//! the SDK `TransactionBuilder` resolves objects, gas, and protocol
//! parameters directly from a fullnode's local state and executes through
//! its transaction orchestrator, without going through a public API.

use iota_macros::sim_test;
use iota_sdk_transaction_builder::{TransactionBuilder, TransactionBuilderClient, WaitForTx};
use iota_sdk_types::{ExecutionStatus, StructTag, Transaction};
use iota_types::effects::TransactionEffectsAPI;
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
        .with(|node| node.transaction_builder_client())
        .expect("fullnodes run a transaction orchestrator");

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

    // Dry run through the builder; gas selection, gas price, and budget
    // estimation all resolve through the node-internal client.
    let mut builder = TransactionBuilder::new(sender).with_client(client.clone());
    builder.send_iota(recipient, 1_000_000u64);
    let dry_run = builder.dry_run(true).await.unwrap();
    assert!(matches!(dry_run.effects.status(), ExecutionStatus::Success));

    // Build, sign, execute, and wait for finalization.
    let mut builder = TransactionBuilder::new(sender).with_client(client.clone());
    builder.send_iota(recipient, 1_000_000u64);
    let transaction = builder.finish().await.unwrap();
    let Transaction::V1(transaction_v1) = &transaction else {
        panic!("the builder produces V1 transactions");
    };
    assert!(transaction_v1.gas_payment.budget > 0);
    assert!(!transaction_v1.gas_payment.objects.is_empty());

    let signed = test_cluster.wallet.sign_transaction(&transaction);
    let signatures = signed.signatures().to_vec();
    let effects = client
        .execute_tx(&signatures, &transaction, WaitForTx::Finalized)
        .await
        .unwrap();
    assert!(matches!(effects.status(), ExecutionStatus::Success));

    // The executed transaction and its effects are readable back from the
    // node's stores.
    let digest = transaction.digest();
    client
        .wait_for_tx(digest, WaitForTx::IndexedOnNode)
        .await
        .unwrap();
    let stored_transaction = client
        .transaction(digest)
        .await
        .unwrap()
        .expect("executed transaction is stored");
    assert_eq!(stored_transaction.transaction.digest(), digest);
    let stored_effects = client
        .transaction_effects(digest)
        .await
        .unwrap()
        .expect("executed transaction effects are stored");
    assert_eq!(
        stored_effects.transaction_digest(),
        effects.transaction_digest()
    );
}
