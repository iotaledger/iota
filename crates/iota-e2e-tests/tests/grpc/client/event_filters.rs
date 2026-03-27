// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! E2E tests for event filters on the gRPC checkpoint stream.
//!
//! A single test cluster is set up, several event-producing transactions are
//! executed, and then multiple event filter scenarios are verified against the
//! same stream range.

use std::time::Duration;

use futures::StreamExt;
use iota_grpc_types::v1::{filter as grpc_filter, types as grpc_types};
use iota_macros::sim_test;
use iota_types::transaction::CallArg;
use tokio::time::timeout;

use super::super::utils::{
    BASICS_PACKAGE, CLOCK_ACCESS_FUNCTION, CLOCK_MODULE, NFT_MINTED_EVENT, NFT_MODULE, NFT_PACKAGE,
    publish_example_package, setup_grpc_test,
};

/// Single test exercising multiple event filter scenarios.
///
/// Setup:
///   1. Publish NFT package (sender_1)
///   2. Publish Basics package (sender_1)
///   3. Mint 2 NFTs (sender_1)  →  2 × NFTMinted events
///   4. Mint 1 NFT (sender_2)   →  1 × NFTMinted event
///   5. clock::access (sender_2) →  1 × TimeEvent
///
/// Total user events: 4
///   - 3 NFTMinted (2 from sender_1, 1 from sender_2)
///   - 1 TimeEvent (from sender_2)
///
/// Scenarios verified:
///   A. Sender filter (sender_1 events only)
///   B. MoveEventType filter (NFTMinted only)
///   C. MovePackageAndModule filter
///   D. Negation filter (NOT sender_1 → only sender_2 events)
///   E. All (AND) — sender_1 AND NFTMinted
///   F. Any (OR) — sender_1 OR NFTMinted
#[sim_test]
async fn test_event_filter_scenarios() {
    let (cluster, client) = setup_grpc_test(None, None).await;

    let sender_1 = cluster.get_address_0();
    let sender_2 = cluster.get_address_1();

    // --- Setup: publish packages and execute event-producing transactions ---

    // 1. Publish NFT package
    let nft_package_id = publish_example_package(&cluster, sender_1, NFT_PACKAGE).await;

    // 2. Publish Basics package
    let basics_package_id = publish_example_package(&cluster, sender_1, BASICS_PACKAGE).await;

    // 3. Mint 2 NFTs from sender_1
    for _ in 0..2 {
        let mint_tx = cluster
            .test_transaction_builder_with_sender(sender_1)
            .await
            .call_nft_create(nft_package_id)
            .build();
        let signed_tx = cluster.sign_transaction(&mint_tx);
        cluster.execute_transaction(signed_tx).await;
    }

    // 4. Mint 1 NFT from sender_2
    let mint_tx = cluster
        .test_transaction_builder_with_sender(sender_2)
        .await
        .call_nft_create(nft_package_id)
        .build();
    let signed_tx = cluster.sign_transaction(&mint_tx);
    cluster.execute_transaction(signed_tx).await;

    // 5. clock::access from sender_2 (emits TimeEvent)
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

    // Wait for checkpoints
    tokio::time::sleep(Duration::from_millis(1500)).await;

    let latest_seq = client
        .get_checkpoint_latest(Some(""), None, None)
        .await
        .expect("get latest checkpoint")
        .body()
        .sequence_number();

    // --- Helper: stream checkpoints with event filter and count events ---
    let stream_and_count_events = |event_filter: grpc_filter::EventFilter| {
        let client = client.clone();
        async move {
            let mut stream = client
                .stream_checkpoints(
                    Some(0),
                    Some(latest_seq),
                    Some("events"),
                    None,
                    Some(event_filter),
                )
                .await
                .expect("Failed to create stream");

            let mut event_count = 0usize;
            timeout(Duration::from_secs(10), async {
                while let Some(result) = stream.body_mut().next().await {
                    let cp = result.expect("stream error");
                    event_count += cp.events().len();
                }
            })
            .await
            .expect("stream timed out");

            event_count
        }
    };

    // --- Scenario A: Sender filter (sender_1 only) ---
    // sender_1 produced: 2 NFTMinted events
    let sender_1_filter = grpc_filter::EventFilter::default().with_sender(
        grpc_filter::AddressFilter::default()
            .with_address(grpc_types::Address::default().with_address(sender_1.to_vec())),
    );
    let count = stream_and_count_events(sender_1_filter.clone()).await;
    assert_eq!(
        count, 2,
        "Scenario A: sender_1 should have 2 events (NFTMinted)"
    );

    // --- Scenario B: MoveEventType filter (NFTMinted) ---
    // 3 NFTMinted events total (2 from sender_1, 1 from sender_2)
    let nft_event_type_filter = grpc_filter::EventFilter::default().with_move_event_type(
        grpc_filter::MoveEventTypeFilter::default().with_struct_tag(format!(
            "{nft_package_id}::{NFT_MODULE}::{NFT_MINTED_EVENT}"
        )),
    );
    let count = stream_and_count_events(nft_event_type_filter.clone()).await;
    assert_eq!(
        count, 3,
        "Scenario B: should match 3 NFTMinted events from both senders"
    );

    // --- Scenario C: MovePackageAndModule filter (basics::clock) ---
    // 1 TimeEvent from clock::access
    let basics_clock_filter = grpc_filter::EventFilter::default().with_move_package_and_module(
        grpc_filter::MovePackageAndModuleFilter::default()
            .with_package_id(
                grpc_types::ObjectId::default().with_object_id(basics_package_id.to_vec()),
            )
            .with_module(CLOCK_MODULE.to_string()),
    );
    let count = stream_and_count_events(basics_clock_filter).await;
    assert_eq!(
        count, 1,
        "Scenario C: should match 1 event from basics::clock"
    );

    // --- Scenario D: Negation (NOT sender_1) ---
    // 0x0 events: 1 DisplayCreated + 1 VersionUpdated = 2
    // sender_2 events: 1 NFTMinted + 1 TimeEvent = 2
    let not_sender_1_filter = grpc_filter::EventFilter::default()
        .with_negation(grpc_filter::NotEventFilter::default().with_filter(sender_1_filter.clone()));
    let count = stream_and_count_events(not_sender_1_filter).await;
    assert_eq!(
        count, 4,
        "Scenario D: NOT sender_1 should match 4 events from 0x0 and sender_2"
    );

    // --- Scenario E: All (AND) — sender_1 AND NFTMinted ---
    // sender_1's NFTMinted events = 2
    let sender_1_and_nft = grpc_filter::EventFilter::default().with_all(
        grpc_filter::AllEventFilter::default()
            .with_filters(vec![sender_1_filter.clone(), nft_event_type_filter.clone()]),
    );
    let count = stream_and_count_events(sender_1_and_nft).await;
    assert_eq!(
        count, 2,
        "Scenario E: sender_1 AND NFTMinted should match 2 events"
    );

    // --- Scenario F: Any (OR) — sender_1 OR NFTMinted ---
    // sender_1 events (2 NFTMinted) ∪ all NFTMinted (3) = 3 unique events
    // (sender_1's 2 NFTMinted are a subset of all 3 NFTMinted)
    let sender_1_or_nft = grpc_filter::EventFilter::default().with_any(
        grpc_filter::AnyEventFilter::default()
            .with_filters(vec![sender_1_filter, nft_event_type_filter]),
    );
    let count = stream_and_count_events(sender_1_or_nft).await;
    assert_eq!(
        count, 3,
        "Scenario F: sender_1 OR NFTMinted should match 3 events"
    );
}
