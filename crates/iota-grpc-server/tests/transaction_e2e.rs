// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::time::Duration;

use futures::StreamExt;
use iota_grpc_types::v0::{common as grpc_common, transactions as grpc_transactions};
use iota_types::effects::TransactionEffectsAPI;
use test_cluster::TestCluster;
use tokio::time::timeout;

mod utils;
use utils::setup_test_cluster_and_client;

async fn setup_test_cluster() -> (
    TestCluster,
    iota_grpc_client::TransactionClient,
    iota_types::base_types::IotaAddress,
) {
    let (cluster, node_client) = setup_test_cluster_and_client().await;

    let transaction_client = node_client
        .transaction_client()
        .expect("Transaction client should be available");

    let sender = cluster.get_address_0();

    (cluster, transaction_client, sender)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_transaction_filtering_and_bcs_serialization() {
    use grpc_transactions::transaction_filter::Filter;

    let (cluster, transaction_client, sender_1) = setup_test_cluster().await;
    let sender_2 = cluster.get_address_1();

    // Client 1: AllTransactionsFilter - should receive all transactions
    let mut all_client = transaction_client.clone();
    let all_filter = grpc_transactions::TransactionFilter {
        filter: Some(Filter::All(grpc_common::AllFilter {})),
    };
    let mut all_stream = all_client
        .stream_transactions(all_filter)
        .await
        .expect("Failed to create all transactions stream");

    // Client 2: FromAddressFilter - should receive only transactions from sender_1
    let mut sender_client = transaction_client.clone();
    let sender_filter = grpc_transactions::TransactionFilter {
        filter: Some(Filter::FromAddress(grpc_common::AddressFilter {
            address: Some(grpc_common::Address {
                address: sender_1.to_vec(),
            }),
        })),
    };
    let mut sender_stream = sender_client
        .stream_transactions(sender_filter)
        .await
        .expect("Failed to create sender transactions stream");

    // Generate transactions after subscription is established
    let cluster_clone = std::sync::Arc::new(cluster);
    let generate_transactions_task = {
        let cluster = cluster_clone.clone();
        tokio::spawn(async move {
            // Wait for the subscription to be established.
            tokio::time::sleep(Duration::from_millis(1000)).await;

            // Generate 2 transactions from sender_1
            for _i in 0..2 {
                let tx = cluster
                    .test_transaction_builder_with_sender(sender_1)
                    .await
                    .transfer_iota(None, sender_2)
                    .build();
                let signed_tx = cluster.sign_transaction(&tx);
                cluster.execute_transaction(signed_tx).await;
                tokio::time::sleep(Duration::from_millis(500)).await;
            }

            // Generate 1 transaction from sender_2
            let tx = cluster
                .test_transaction_builder_with_sender(sender_2)
                .await
                .transfer_iota(None, sender_1)
                .build();
            let signed_tx = cluster.sign_transaction(&tx);
            cluster.execute_transaction(signed_tx).await;
            tokio::time::sleep(Duration::from_millis(500)).await;

            // Wait a bit more to ensure all transactions are processed
            tokio::time::sleep(Duration::from_millis(2000)).await;
        })
    };

    // Concurrently collect transactions from both clients
    let all_transactions_task = tokio::spawn(async move {
        let mut all_transactions = Vec::new();

        let result = timeout(Duration::from_secs(30), async {
            while let Some(transaction_result) = all_stream.next().await {
                match transaction_result {
                    Ok(transaction) => {
                        // Verify transaction data integrity
                        assert!(!transaction.transaction_digest().to_string().is_empty());

                        all_transactions.push(transaction);

                        if all_transactions.len() >= 3 {
                            break;
                        }
                    }
                    Err(e) => panic!("AllTransactionsFilter client error: {e}"),
                }
            }
        })
        .await;

        assert!(
            result.is_ok(),
            "AllTransactionsFilter should receive transactions"
        );
        (all_transactions.len(), all_transactions)
    });

    let sender_transactions_task = tokio::spawn(async move {
        let mut sender_transactions = Vec::new();

        let result = timeout(Duration::from_secs(30), async {
            while let Some(transaction_result) = sender_stream.next().await {
                match transaction_result {
                    Ok(transaction) => {
                        // Verify transaction data integrity
                        assert!(!transaction.transaction_digest().to_string().is_empty());

                        sender_transactions.push(transaction);

                        if sender_transactions.len() >= 2 {
                            break;
                        }
                    }
                    Err(e) => panic!("FromAddressFilter client error: {e}"),
                }
            }
        })
        .await;

        assert!(
            result.is_ok(),
            "FromAddressFilter should receive transactions"
        );
        (sender_transactions.len(), sender_transactions)
    });

    // Wait for all tasks to finish
    let (all_results, sender_results, generate_result) = tokio::join!(
        all_transactions_task,
        sender_transactions_task,
        generate_transactions_task
    );
    let (all_count, _all_transactions) =
        all_results.expect("AllTransactionsFilter task should complete");
    let (sender_count, _sender_transactions) =
        sender_results.expect("FromAddressFilter task should complete");
    generate_result.expect("Generate transactions task should complete");

    // Verify individual filter behaviors:
    // - AllTransactionsFilter: receives all transactions (2 from sender_1 + 1 from
    //   sender_2 = 3)
    // - FromAddressFilter: receives only transactions from sender_1 (2
    //   transactions)
    assert_eq!(
        all_count, 3,
        "AllTransactionsFilter should receive all 3 transactions"
    );
    assert_eq!(
        sender_count, 2,
        "FromAddressFilter should receive 2 transactions from sender_1"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_transaction_kind_filtering() {
    use grpc_transactions::transaction_filter::Filter;

    let (cluster, transaction_client, sender) = setup_test_cluster().await;

    // Test TransactionKind filter
    let mut kind_client = transaction_client.clone();
    let kind_filter = grpc_transactions::TransactionFilter {
        filter: Some(Filter::TransactionKind(
            grpc_transactions::TransactionKindFilter {
                kind: grpc_transactions::TransactionKind::ProgrammableTransaction as i32,
            },
        )),
    };
    let mut kind_stream = kind_client
        .stream_transactions(kind_filter)
        .await
        .expect("Failed to create transaction kind stream");

    // Generate a programmable transaction
    let cluster_clone = std::sync::Arc::new(cluster);
    let generate_transaction_task = {
        let cluster = cluster_clone.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(1000)).await;

            let tx = cluster
                .test_transaction_builder_with_sender(sender)
                .await
                .transfer_iota(None, cluster.get_address_1())
                .build();
            let signed_tx = cluster.sign_transaction(&tx);
            cluster.execute_transaction(signed_tx).await;

            tokio::time::sleep(Duration::from_millis(2000)).await;
        })
    };

    let kind_transactions_task = tokio::spawn(async move {
        let mut transactions = Vec::new();

        let result = timeout(Duration::from_secs(20), async {
            if let Some(transaction_result) = kind_stream.next().await {
                match transaction_result {
                    Ok(transaction) => {
                        assert!(!transaction.transaction_digest().to_string().is_empty());
                        transactions.push(transaction);
                    }
                    Err(e) => panic!("TransactionKind filter error: {e}"),
                }
            }
        })
        .await;

        assert!(
            result.is_ok(),
            "TransactionKind filter should receive transactions"
        );
        transactions.len()
    });

    let (generate_result, kind_count_result) =
        tokio::join!(generate_transaction_task, kind_transactions_task);
    generate_result.expect("Generate transaction task should complete");
    let kind_count = kind_count_result.expect("Kind transactions task should complete");
    assert_eq!(
        kind_count, 1,
        "TransactionKind filter should receive 1 programmable transaction"
    );
}
