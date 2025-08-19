// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::time::Duration;

use futures::StreamExt;
use iota_config::local_ip_utils;
use iota_grpc_api::{
    client::{EventClient, NodeClient},
    events::{
        AllFilter, AndFilter, EventFilter, MoveEventTypeFilter, OrFilter, PackageFilter,
        SenderFilter, event_filter::Filter,
    },
};
use iota_types::{base_types::ObjectID, effects::TransactionEffectsAPI, transaction::CallArg};
use test_cluster::{TestCluster, TestClusterBuilder};
use tokio::time::timeout;

// Test constants for Move packages and contracts
const NFT_PACKAGE: &str = "nft";
const BASICS_PACKAGE: &str = "basics";
const NFT_MODULE: &str = "testnet_nft";
const CLOCK_MODULE: &str = "clock";
const CLOCK_ACCESS_FUNCTION: &str = "access";

// Event type names
const NFT_MINTED_EVENT: &str = "NFTMinted";
const TIME_EVENT: &str = "TimeEvent";

// JSON field names for event validation
const OBJECT_ID_FIELD: &str = "object_id";
const CREATOR_FIELD: &str = "creator";
const NAME_FIELD: &str = "name";
const TIMESTAMP_MS_FIELD: &str = "timestamp_ms";

async fn setup_test_cluster() -> (TestCluster, EventClient, ObjectID, ObjectID) {
    let localhost = local_ip_utils::localhost_for_testing();
    let grpc_port = local_ip_utils::get_available_port(&localhost);
    let grpc_addr = format!("{localhost}:{grpc_port}");

    let cluster = TestClusterBuilder::new()
        .with_fullnode_grpc_api_address(grpc_addr.parse().expect("Invalid gRPC address"))
        .disable_fullnode_pruning()
        .with_num_validators(1)
        .build()
        .await;

    let client = NodeClient::connect(&format!("http://{grpc_addr}"))
        .await
        .expect("Failed to connect to gRPC");

    let event_client = client
        .event_client()
        .expect("Event client should be available");

    let sender = cluster.get_address_0();

    // Publish NFT package
    let nft_tx = cluster
        .test_transaction_builder_with_sender(sender)
        .await
        .publish_examples(NFT_PACKAGE)
        .build();

    let signed_tx = cluster.sign_transaction(&nft_tx);
    let (effects, _events) = cluster
        .execute_transaction_return_raw_effects(signed_tx)
        .await
        .expect("NFT publishing should succeed");

    let nft_package_id = effects
        .created()
        .iter()
        .find(|obj| obj.1.is_immutable())
        .map(|obj| obj.0.0)
        .expect("Should have created NFT package");

    // Publish basics package
    let basics_tx = cluster
        .test_transaction_builder_with_sender(sender)
        .await
        .publish_examples(BASICS_PACKAGE)
        .build();

    let signed_tx = cluster.sign_transaction(&basics_tx);
    let (effects, _events) = cluster
        .execute_transaction_return_raw_effects(signed_tx)
        .await
        .expect("Basics publishing should succeed");

    let basics_package_id = effects
        .created()
        .iter()
        .find(|obj| obj.1.is_immutable())
        .map(|obj| obj.0.0)
        .expect("Should have created basics package");

    (cluster, event_client, nft_package_id, basics_package_id)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_event_filtering_and_bcs_serialization() {
    let (cluster, event_client, nft_package_id, basics_package_id) = setup_test_cluster().await;
    let sender_1 = cluster.get_address_0();
    let sender_2 = cluster.get_address_1();

    // Client 1: AllFilter - should receive all events
    let mut all_client = event_client.clone();
    let all_filter = EventFilter {
        filter: Some(Filter::All(AllFilter {})),
    };
    let mut all_stream = all_client
        .stream_events(all_filter)
        .await
        .expect("Failed to create all events stream");

    // Client 2: Complex nested filter - ((Package AND MoveEventType) OR Sender)
    // This should demonstrate sophisticated boolean logic
    let mut nested_client = event_client.clone();
    let nested_filter = EventFilter {
        filter: Some(Filter::Or(OrFilter {
            filters: vec![
                // Branch 1: Package AND MoveEventType (matches all NFT events)
                EventFilter {
                    filter: Some(Filter::And(AndFilter {
                        filters: vec![
                            EventFilter {
                                filter: Some(Filter::Package(PackageFilter {
                                    package_id: nft_package_id.to_hex_literal(),
                                })),
                            },
                            EventFilter {
                                filter: Some(Filter::MoveEventType(MoveEventTypeFilter {
                                    address: nft_package_id.to_hex_literal(),
                                    module: NFT_MODULE.to_string(),
                                    name: NFT_MINTED_EVENT.to_string(),
                                })),
                            },
                        ],
                    })),
                },
                // Branch 2: Sender filter (matches events from sender_1)
                EventFilter {
                    filter: Some(Filter::Sender(SenderFilter {
                        sender: sender_1.to_string(),
                    })),
                },
            ],
        })),
    };
    let mut nested_stream = nested_client
        .stream_events(nested_filter)
        .await
        .expect("Failed to create nested events stream");

    // Generate events after subscription is established
    let cluster_clone = std::sync::Arc::new(cluster);
    let generate_events_task = {
        let cluster = cluster_clone.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(1000)).await;

            // Generate 2 NFT events from sender_1 -> MATCH nested filter (both branches)
            for _i in 0..2 {
                let nft_tx = cluster
                    .test_transaction_builder_with_sender(sender_1)
                    .await
                    .call_nft_create(nft_package_id)
                    .build();
                let signed_tx = cluster.sign_transaction(&nft_tx);
                cluster.execute_transaction(signed_tx).await;
                tokio::time::sleep(Duration::from_millis(300)).await;
            }

            // Generate 1 NFT event from sender_2 -> MATCH nested filter (branch 1 only:
            // Package AND EventType)
            let nft_tx = cluster
                .test_transaction_builder_with_sender(sender_2)
                .await
                .call_nft_create(nft_package_id)
                .build();
            let signed_tx = cluster.sign_transaction(&nft_tx);
            cluster.execute_transaction(signed_tx).await;
            tokio::time::sleep(Duration::from_millis(300)).await;

            // Generate 1 TimeEvent using clock::access from basics package
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
            tokio::time::sleep(Duration::from_millis(300)).await;

            // Wait a bit more to ensure all events are processed
            tokio::time::sleep(Duration::from_millis(2000)).await;
        })
    };

    // Concurrently collect events from both clients
    let all_events_task = tokio::spawn(async move {
        let mut all_count = 0;
        let mut all_events = Vec::new();

        let result = timeout(Duration::from_secs(15), async {
            while let Some(event_result) = all_stream.next().await {
                match event_result {
                    Ok(event) => {
                        all_count += 1;

                        // Verify BCS serialization integrity
                        assert!(event.timestamp_ms.is_some());
                        assert!(!event.bcs.bytes().is_empty());
                        // AllFilter should receive both NFT and TimeEvent events
                        assert!(
                            event.package_id == nft_package_id
                                || event.package_id == basics_package_id,
                            "AllFilter should receive events from both packages"
                        );

                        all_events.push(event);

                        if all_count >= 4 {
                            break;
                        }
                    }
                    Err(e) => panic!("AllFilter client error: {}", e),
                }
            }
        })
        .await;

        assert!(result.is_ok(), "AllFilter should receive events");
        (all_count, all_events)
    });

    let nested_events_task = tokio::spawn(async move {
        let mut nested_count = 0;
        let mut nested_events = Vec::new();

        let result = timeout(Duration::from_secs(15), async {
            while let Some(event_result) = nested_stream.next().await {
                match event_result {
                    Ok(event) => {
                        nested_count += 1;

                        // Verify nested filter logic: ((Package AND MoveEventType) OR Sender)
                        let is_nft_event = event.package_id == nft_package_id
                            && event.type_.name.as_ident_str().as_str() == NFT_MINTED_EVENT;
                        let is_sender1_event = event.sender == sender_1;

                        assert!(
                            is_nft_event || is_sender1_event,
                            "Nested filter should match NFT events OR sender_1 events"
                        );
                        assert!(!event.bcs.bytes().is_empty(), "BCS data must be valid");

                        // Verify JSON structure for BCS deserialization
                        let parsed_json = &event.parsed_json;
                        if event.type_.name.as_ident_str().as_str() == NFT_MINTED_EVENT {
                            assert!(parsed_json.get(OBJECT_ID_FIELD).is_some());
                            assert!(parsed_json.get(CREATOR_FIELD).is_some());
                            assert!(parsed_json.get(NAME_FIELD).is_some());
                        } else if event.type_.name.as_ident_str().as_str() == TIME_EVENT {
                            assert!(parsed_json.get(TIMESTAMP_MS_FIELD).is_some());
                        }

                        nested_events.push(event);

                        if nested_count >= 3 {
                            break;
                        }
                    }
                    Err(e) => panic!("Nested OR(AND, Sender) filter client error: {}", e),
                }
            }
        })
        .await;

        assert!(result.is_ok(), "Nested filter should receive events");
        (nested_count, nested_events)
    });

    // Wait for all tasks to finish
    let (all_results, nested_results, _) =
        tokio::join!(all_events_task, nested_events_task, generate_events_task);
    let (all_count, _all_events) = all_results.expect("AllFilter task should complete");
    let (nested_count, _nested_events) =
        nested_results.expect("Nested filter task should complete");

    // Verify complex nested AND/OR filter works correctly
    // Nested Filter: ((Package AND MoveEventType) OR Sender)
    // Event filtering analysis:
    // - Events 1-3: NFT from sender_1 -> MATCH (branch 2: Sender)
    // - Event 4: NFT from sender_2 -> MATCH (branch 1: Package AND EventType)
    // - Event 4: TimeEvent from sender_2 -> NO MATCH (neither branch matches
    //   TimeEvent type)
    // This demonstrates filtering precision: 3/4 events match the nested logic
    assert_eq!(all_count, 4, "AllFilter should receive all 4 events");
    assert_eq!(
        nested_count, 3,
        "Complex nested OR(AND(Package,EventType), Sender) filter should receive 3/4 events, filtering out TimeEvent events"
    );
}
