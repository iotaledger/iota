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
    publish_example_package, setup_grpc_test, wait_for_executed_transactions_checkpointed,
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
async fn event_filter_scenarios() {
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
            vec![CallArg::CLOCK_IMMUTABLE],
        )
        .build();
    let signed_tx = cluster.sign_transaction(&clock_tx);
    cluster.execute_transaction(signed_tx).await;

    let latest_seq = wait_for_executed_transactions_checkpointed(&cluster, &client).await;

    // --- Helper: stream checkpoints with event filter and collect events ---
    let stream_and_collect_events = |event_filter: grpc_filter::EventFilter| {
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

            let mut events: Vec<iota_grpc_types::v1::event::Event> = Vec::new();
            timeout(Duration::from_secs(10), async {
                while let Some(result) = stream.body_mut().next().await {
                    let cp = result.expect("stream error");
                    events.extend(cp.events().iter().cloned());
                }
            })
            .await
            .expect("stream timed out");

            events
        }
    };

    // --- Scenario A: Sender filter (sender_1 only) ---
    // sender_1 produced: 2 NFTMinted events
    let sender_1_filter = grpc_filter::EventFilter::default().with_sender(
        grpc_filter::AddressFilter::default().with_address(
            grpc_types::Address::default().with_address(sender_1.into_bytes().to_vec()),
        ),
    );
    let events = stream_and_collect_events(sender_1_filter.clone()).await;
    assert_eq!(
        events.len(),
        2,
        "Scenario A: sender_1 should have 2 events (NFTMinted)"
    );
    for event in &events {
        assert!(
            event.bcs_contents.is_some(),
            "Scenario A: BCS data must be valid"
        );
        assert_eq!(
            event.sender.as_ref().unwrap().address.as_ref(),
            sender_1.as_bytes(),
            "Scenario A: events must come from sender_1"
        );
    }

    // --- Scenario B: MoveEventType filter (NFTMinted) ---
    // 3 NFTMinted events total (2 from sender_1, 1 from sender_2)
    let nft_event_type_filter = grpc_filter::EventFilter::default().with_move_event_type(
        grpc_filter::MoveEventTypeFilter::default().with_struct_tag(format!(
            "{nft_package_id}::{NFT_MODULE}::{NFT_MINTED_EVENT}"
        )),
    );
    let events = stream_and_collect_events(nft_event_type_filter.clone()).await;
    assert_eq!(
        events.len(),
        3,
        "Scenario B: should match 3 NFTMinted events from both senders"
    );
    for event in &events {
        assert!(
            event.bcs_contents.is_some(),
            "Scenario B: BCS data must be valid"
        );
        assert_eq!(
            event.package_id.as_ref().unwrap().object_id.as_ref(),
            nft_package_id.as_bytes(),
            "Scenario B: events must come from the NFT package"
        );
    }

    // --- Scenario C: MovePackageAndModule filter (basics::clock) ---
    // 1 TimeEvent from clock::access
    let basics_clock_filter = grpc_filter::EventFilter::default().with_move_package_and_module(
        grpc_filter::MovePackageAndModuleFilter::default()
            .with_package_id(
                grpc_types::ObjectId::default()
                    .with_object_id(basics_package_id.into_bytes().to_vec()),
            )
            .with_module(CLOCK_MODULE.to_string()),
    );
    let events = stream_and_collect_events(basics_clock_filter).await;
    assert_eq!(
        events.len(),
        1,
        "Scenario C: should match 1 event from basics::clock"
    );

    // --- Scenario D: Negation (NOT sender_1) ---
    // Assert what the filter itself guarantees rather than the total event
    // count: no sender_1 event may leak through, and the application events
    // from sender_2 (1 NFTMinted + 1 TimeEvent) must be present.  System
    // events emitted from 0x0 are intentionally not counted here so the test
    // stays stable if the framework's set of system events changes.
    let not_sender_1_filter = grpc_filter::EventFilter::default()
        .with_negation(grpc_filter::NotEventFilter::default().with_filter(sender_1_filter.clone()));
    let events = stream_and_collect_events(not_sender_1_filter).await;
    assert!(
        events
            .iter()
            .all(|e| { e.sender.as_ref().map(|s| s.address.as_ref()) != Some(sender_1.as_ref()) }),
        "Scenario D: NOT sender_1 must exclude every sender_1 event"
    );
    let sender_2_count = events
        .iter()
        .filter(|e| e.sender.as_ref().map(|s| s.address.as_ref()) == Some(sender_2.as_ref()))
        .count();
    assert_eq!(
        sender_2_count, 2,
        "Scenario D: sender_2 should contribute 2 events (1 NFTMinted + 1 TimeEvent)"
    );

    // --- Scenario E: All (AND) — sender_1 AND NFTMinted ---
    // sender_1's NFTMinted events = 2
    let sender_1_and_nft = grpc_filter::EventFilter::default().with_all(
        grpc_filter::AllEventFilter::default()
            .with_filters(vec![sender_1_filter.clone(), nft_event_type_filter.clone()]),
    );
    let events = stream_and_collect_events(sender_1_and_nft).await;
    assert_eq!(
        events.len(),
        2,
        "Scenario E: sender_1 AND NFTMinted should match 2 events"
    );

    // --- Scenario F: Any (OR) — sender_1 OR NFTMinted ---
    // sender_1 events (2 NFTMinted) ∪ all NFTMinted (3) = 3 unique events
    // (sender_1's 2 NFTMinted are a subset of all 3 NFTMinted)
    let sender_1_or_nft = grpc_filter::EventFilter::default().with_any(
        grpc_filter::AnyEventFilter::default()
            .with_filters(vec![sender_1_filter, nft_event_type_filter]),
    );
    let events = stream_and_collect_events(sender_1_or_nft).await;
    assert_eq!(
        events.len(),
        3,
        "Scenario F: sender_1 OR NFTMinted should match 3 events"
    );
    for event in &events {
        assert!(
            event.bcs_contents.is_some(),
            "Scenario F: BCS data must be valid"
        );
        let sender_matches =
            event.sender.as_ref().map(|s| s.address.as_ref()) == Some(sender_1.as_ref());
        let nft_matches = event.package_id.as_ref().map(|p| p.object_id.as_ref())
            == Some(nft_package_id.as_ref());
        assert!(
            sender_matches || nft_matches,
            "Scenario F: events must match at least one sub-filter: package_id={:?}",
            event.package_id.as_ref().map(|p| &p.object_id)
        );
    }
}
