// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! E2E tests for transaction filters on the gRPC checkpoint stream.
//!
//! A single test cluster is set up, several transaction types are executed, and
//! then multiple filter scenarios are verified against the same stream range to
//! avoid spawning the test framework multiple times.

use std::time::Duration;

use futures::StreamExt;
use iota_grpc_types::v1::{filter as grpc_filter, types as grpc_types};
use iota_macros::sim_test;
use iota_types::transaction::CallArg;
use tokio::time::timeout;

use super::super::utils::{
    BASICS_PACKAGE, CLOCK_ACCESS_FUNCTION, CLOCK_MODULE, NFT_PACKAGE, publish_example_package,
    setup_grpc_test,
};

/// Single test exercising multiple transaction filter scenarios.
///
/// Setup:
///   1. Publish NFT package (sender_1)  →  Publish command
///   2. Publish Basics package (sender_1)  →  Publish command
///   3. Mint NFT (sender_1)  →  MoveCall command (with events)
///   4. Transfer IOTA (sender_2)  →  TransferObjects + SplitCoins commands
///   5. MoveCall clock::access (sender_2)  →  MoveCall command (with events)
///
/// Scenarios verified:
///   A. Sender filter (sender_1 only)
///   B. Command filter — Publish
///   C. Command filter — MoveCall with package/module
///   D. Execution status — Success
///   E. Combined: Sender AND Command
///   F. Any (OR) of two Command filters
#[sim_test]
async fn test_transaction_filter_scenarios() {
    let (cluster, client) = setup_grpc_test(None, None).await;

    let sender_1 = cluster.get_address_0();
    let sender_2 = cluster.get_address_1();

    // --- Setup: execute transactions ---

    // 1. Publish NFT package (sender_1)
    let nft_package_id = publish_example_package(&cluster, sender_1, NFT_PACKAGE).await;

    // 2. Publish Basics package (sender_1)
    let basics_package_id = publish_example_package(&cluster, sender_1, BASICS_PACKAGE).await;

    // 3. Mint NFT (sender_1) — MoveCall to nft_package
    let mint_tx = cluster
        .test_transaction_builder_with_sender(sender_1)
        .await
        .call_nft_create(nft_package_id)
        .build();
    let signed_tx = cluster.sign_transaction(&mint_tx);
    cluster.execute_transaction(signed_tx).await;

    // 4. Transfer IOTA (sender_2) — generates TransferObjects + SplitCoins
    let transfer_tx = cluster
        .test_transaction_builder_with_sender(sender_2)
        .await
        .transfer_iota(Some(100), sender_1)
        .build();
    let signed_tx = cluster.sign_transaction(&transfer_tx);
    cluster.execute_transaction(signed_tx).await;

    // 5. MoveCall clock::access (sender_2)
    let clock_tx = cluster
        .test_transaction_builder_with_sender(sender_2)
        .await
        .move_call(
            basics_package_id,
            CLOCK_MODULE,
            CLOCK_ACCESS_FUNCTION,
            vec![CallArg::CLOCK_IMM],
        )
        .build();
    let signed_tx = cluster.sign_transaction(&clock_tx);
    cluster.execute_transaction(signed_tx).await;

    // Wait for all transactions to land in checkpoints
    tokio::time::sleep(Duration::from_millis(1500)).await;

    let latest_seq = client
        .get_checkpoint_latest(Some(""), None, None)
        .await
        .expect("get latest checkpoint")
        .body()
        .sequence_number();

    // --- Helper closure to stream and collect matching transactions ---
    let stream_and_collect = |tx_filter: grpc_filter::TransactionFilter| {
        let client = client.clone();
        async move {
            let mut stream = client
                .stream_checkpoints(
                    Some(0),
                    Some(latest_seq),
                    Some("transactions.transaction.bcs,transactions.effects.bcs"),
                    Some(tx_filter),
                    None,
                )
                .await
                .expect("Failed to create stream");

            let mut tx_count = 0usize;
            timeout(Duration::from_secs(10), async {
                while let Some(result) = stream.body_mut().next().await {
                    let cp = result.expect("stream error");
                    tx_count += cp.executed_transactions.len();
                }
            })
            .await
            .expect("stream timed out");

            tx_count
        }
    };

    // --- Scenario A: Sender filter (sender_1) ---
    // sender_1 did: publish NFT, publish basics, mint NFT = 3 transactions
    let sender_filter = grpc_filter::TransactionFilter::default().with_sender(
        grpc_filter::AddressFilter::default()
            .with_address(grpc_types::Address::default().with_address(sender_1.to_vec())),
    );
    let count = stream_and_collect(sender_filter).await;
    assert_eq!(count, 3, "Scenario A: sender_1 should have 3 transactions");

    // --- Scenario B: Command filter — Publish ---
    // 2 publish transactions (NFT + Basics)
    let publish_filter = grpc_filter::TransactionFilter::default().with_command(
        grpc_filter::CommandFilter::default()
            .with_publish(grpc_filter::PublishCommandFilter::default()),
    );
    let count = stream_and_collect(publish_filter).await;
    assert_eq!(count, 2, "Scenario B: should match 2 Publish transactions");

    // --- Scenario C: Command filter — MoveCall with package ---
    // MoveCall to nft_package: mint NFT (1 tx)
    let move_call_filter = grpc_filter::TransactionFilter::default().with_command(
        grpc_filter::CommandFilter::default().with_move_call(
            grpc_filter::MoveCallCommandFilter::default().with_package_id(
                grpc_types::ObjectId::default().with_object_id(nft_package_id.to_vec()),
            ),
        ),
    );
    let count = stream_and_collect(move_call_filter).await;
    assert_eq!(
        count, 1,
        "Scenario C: should match 1 MoveCall to NFT package"
    );

    // --- Scenario D: Execution status — Success ---
    // All 5 user transactions should succeed (system transactions too, but
    // we filter by ProgrammableTransaction kind to count only user txns)
    let success_and_programmable = grpc_filter::TransactionFilter::default().with_all(
        grpc_filter::AllTransactionFilter::default().with_filters(vec![
            grpc_filter::TransactionFilter::default().with_execution_status(
                grpc_filter::ExecutionStatusFilter::default().with_success(true),
            ),
            grpc_filter::TransactionFilter::default().with_transaction_kinds(
                grpc_filter::TransactionKindsFilter::default().with_kinds(vec![
                    grpc_filter::TransactionKind::ProgrammableTransaction.into(),
                ]),
            ),
        ]),
    );
    let count = stream_and_collect(success_and_programmable).await;
    assert_eq!(
        count, 5,
        "Scenario D: all 5 programmable transactions should be successful"
    );

    // --- Scenario E: Combined — sender_1 AND Publish ---
    // sender_1 published 2 packages
    let sender_and_publish = grpc_filter::TransactionFilter::default().with_all(
        grpc_filter::AllTransactionFilter::default().with_filters(vec![
            grpc_filter::TransactionFilter::default().with_sender(
                grpc_filter::AddressFilter::default()
                    .with_address(grpc_types::Address::default().with_address(sender_1.to_vec())),
            ),
            grpc_filter::TransactionFilter::default().with_command(
                grpc_filter::CommandFilter::default()
                    .with_publish(grpc_filter::PublishCommandFilter::default()),
            ),
        ]),
    );
    let count = stream_and_collect(sender_and_publish).await;
    assert_eq!(
        count, 2,
        "Scenario E: sender_1 AND Publish should match 2 transactions"
    );

    // --- Scenario F: Any (OR) of Publish or MoveCall to basics ---
    // Publish (2) + MoveCall to basics/clock (1) = 3 unique transactions
    // Note: publish basics is already counted in Publish
    let any_filter = grpc_filter::TransactionFilter::default().with_any(
        grpc_filter::AnyTransactionFilter::default().with_filters(vec![
            grpc_filter::TransactionFilter::default().with_command(
                grpc_filter::CommandFilter::default()
                    .with_publish(grpc_filter::PublishCommandFilter::default()),
            ),
            grpc_filter::TransactionFilter::default().with_command(
                grpc_filter::CommandFilter::default().with_move_call(
                    grpc_filter::MoveCallCommandFilter::default().with_package_id(
                        grpc_types::ObjectId::default().with_object_id(basics_package_id.to_vec()),
                    ),
                ),
            ),
        ]),
    );
    let count = stream_and_collect(any_filter).await;
    assert_eq!(
        count, 3,
        "Scenario F: Publish OR MoveCall(basics) should match 3 transactions"
    );
}
