// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
use std::time::Duration;

use futures::StreamExt;
use iota_grpc_types::v1::{filter as grpc_filter, types as grpc_types};
use iota_types::transaction::CallArg;
use tokio::time::timeout;

use crate::utils::{
    BASICS_PACKAGE, CLOCK_ACCESS_FUNCTION, CLOCK_MODULE, NFT_MINTED_EVENT, NFT_MODULE, NFT_PACKAGE,
    publish_example_package, setup_grpc_test,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_get_checkpoint() {
    let (_cluster, client) = setup_grpc_test(Some(2), None).await;

    // Test getting checkpoint data for sequence number 0
    let response = client
        .get_checkpoint_by_sequence_number(
            0,
            Some("checkpoint.summary,checkpoint.contents,transactions"),
            None,
            None,
        )
        .await
        .expect("gRPC call");

    // Verify the checkpoint data structure
    assert_eq!(response.body().sequence_number(), 0);
    let summary = response
        .body()
        .summary()
        .expect("should have summary")
        .summary()
        .expect("should deserialize summary");
    assert_eq!(summary.epoch, 0);
    assert!(!response.body().executed_transactions.is_empty());
    assert!(response.body().contents.is_some());
    let digest_0 = summary.content_digest;

    // Test getting another checkpoint
    let response_1 = client
        .get_checkpoint_by_sequence_number(1, Some("checkpoint.summary"), None, None)
        .await
        .expect("gRPC call");

    assert_eq!(response_1.body().sequence_number(), 1);
    let summary_1 = response_1
        .body()
        .summary()
        .expect("should have summary")
        .summary()
        .expect("should deserialize summary");
    assert_eq!(summary_1.epoch, 0);
    let digest_1 = summary_1.content_digest;

    // Verify they are different checkpoints
    assert_ne!(digest_0, digest_1);

    // Test getting checkpoint data for a non-existent sequence number
    match client
        .get_checkpoint_by_sequence_number(999999, None, None, None)
        .await
    {
        Ok(_) => {
            panic!("Unexpectedly found checkpoint data for non-existent sequence number");
        }
        Err(iota_grpc_client::Error::Grpc(status)) => {
            assert_eq!(status.code(), tonic::Code::NotFound);
        }
        Err(e) => {
            panic!("Unexpected error type: {e:?}");
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_stream_checkpoints() {
    let (_cluster, client) = setup_grpc_test(None, None).await;

    let mut stream = client
        .stream_checkpoints(None, Some(2), None, None, None)
        .await
        .unwrap();

    tokio::time::timeout(Duration::from_secs(120), async {
        if let Some(res) = stream.body_mut().next().await {
            match res {
                Ok(response) => {
                    assert_eq!(response.sequence_number(), 2);
                }
                Err(e) => {
                    panic!("Stream error: {e:?}");
                }
            }
        } else {
            panic!("No checkpoint data returned");
        }
    })
    .await
    .expect("waiting for checkpoint data timed out");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_event_filtering() {
    let (cluster, client) = setup_grpc_test(None, None).await;

    let sender_1 = cluster.get_address_0();
    let sender_2 = cluster.get_address_1();

    // Publish NFT and basics packages
    let nft_package_id = publish_example_package(&cluster, sender_1, NFT_PACKAGE).await;
    let basics_package_id = publish_example_package(&cluster, sender_1, BASICS_PACKAGE).await;

    // Define event filters for later use
    let sender_filter = grpc_filter::EventFilter::default().with_sender(
        grpc_filter::AddressFilter::default()
            .with_address(grpc_types::Address::default().with_address(sender_1.to_vec())),
    );

    let nft_filter = grpc_filter::EventFilter::default().with_move_event_type(
        grpc_filter::MoveEventTypeFilter::default().with_struct_tag(format!(
            "{nft_package_id}::{NFT_MODULE}::{NFT_MINTED_EVENT}"
        )),
    );

    let any_filter = grpc_filter::EventFilter::default().with_any(
        grpc_filter::AnyEventFilter::default()
            .with_filters(vec![sender_filter.clone(), nft_filter.clone()]),
    );

    // Generate all events first before streaming
    // Generate 2 NFT events from sender_1
    for _i in 0..2 {
        let nft_tx = cluster
            .test_transaction_builder_with_sender(sender_1)
            .await
            .call_nft_create(nft_package_id)
            .build();
        let signed_tx = cluster.sign_transaction(&nft_tx);
        cluster.execute_transaction(signed_tx).await;
    }

    // Generate 1 NFT event from sender_2
    let nft_tx = cluster
        .test_transaction_builder_with_sender(sender_2)
        .await
        .call_nft_create(nft_package_id)
        .build();
    let signed_tx = cluster.sign_transaction(&nft_tx);
    cluster.execute_transaction(signed_tx).await;

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

    // Wait for checkpoints to include all transactions
    tokio::time::sleep(Duration::from_millis(1500)).await;

    // get the latest checkpoint sequence number
    let latest_checkpoint_seq = client
        .get_checkpoint_latest(Some(""), None, None)
        .await
        .expect("Failed to get latest checkpoint")
        .body()
        .sequence_number();

    // Test 1: SenderFilter - should receive only events from sender_1
    let mut sender_stream = client
        .stream_checkpoints(
            Some(0),
            Some(latest_checkpoint_seq),
            Some("events"),
            None,
            Some(sender_filter),
        )
        .await
        .expect("Failed to create sender events stream");

    let mut sender_events = Vec::new();
    let result = timeout(Duration::from_secs(5), async {
        while let Some(checkpoint_result) = sender_stream.body_mut().next().await {
            match checkpoint_result {
                Ok(response) => {
                    let events = response.events();
                    for event in events {
                        // Verify BCS serialization integrity
                        assert!(event.bcs_contents.is_some(), "BCS data must be valid");
                        // Verify sender filter logic: only events from sender_1
                        assert_eq!(
                            event.sender.as_ref().unwrap().address.as_ref(),
                            sender_1.as_ref(),
                            "SenderFilter should only match sender_1 events"
                        );

                        sender_events.push(event.clone());
                    }
                }
                Err(e) => panic!("SenderFilter client error: {e}"),
            }
        }
    })
    .await;

    assert!(result.is_ok(), "SenderFilter should receive events");
    assert_eq!(
        sender_events.len(),
        2,
        "SenderFilter should receive 2 events from sender_1"
    );

    // Test 2: MoveEventTypeFilter - should receive only NFT events
    let mut nft_stream = client
        .stream_checkpoints(
            Some(0),
            Some(latest_checkpoint_seq),
            Some("events"),
            None,
            Some(nft_filter),
        )
        .await
        .expect("Failed to create NFT events stream");

    let mut nft_events = Vec::new();
    let result = timeout(Duration::from_secs(5), async {
        while let Some(checkpoint_result) = nft_stream.body_mut().next().await {
            match checkpoint_result {
                Ok(response) => {
                    let events = response.events();
                    for event in events {
                        // Verify BCS serialization integrity
                        assert!(event.bcs_contents.is_some(), "BCS data must be valid");

                        // Verify NFT filter logic: only NFT events
                        assert_eq!(
                            event.package_id.as_ref().unwrap().object_id.as_ref(),
                            nft_package_id.as_ref(),
                            "MoveEventTypeFilter should only match NFT package events"
                        );

                        nft_events.push(event.clone());
                    }
                }
                Err(e) => panic!("MoveEventTypeFilter client error: {e}"),
            }
        }
    })
    .await;

    assert!(result.is_ok(), "MoveEventTypeFilter should receive events");
    assert_eq!(
        nft_events.len(),
        3,
        "MoveEventTypeFilter should receive 3 NFT events from both senders"
    );

    // Test 3: AnyEventFilter - should receive sender_1 events, and all NFT events
    let mut any_stream = client
        .stream_checkpoints(
            Some(0),
            Some(latest_checkpoint_seq),
            Some("events"),
            None,
            Some(any_filter),
        )
        .await
        .expect("Failed to create all events stream");

    let mut any_events = Vec::new();
    let result = timeout(Duration::from_secs(5), async {
        while let Some(checkpoint_result) = any_stream.body_mut().next().await {
            match checkpoint_result {
                Ok(response) => {
                    let events = response.events();
                    for event in events {
                        // Verify BCS serialization integrity
                        assert!(event.bcs_contents.is_some());

                        // Verify AnyEventFilter logic: events from sender_1 and NFT events
                        assert!(
                            (event.sender.as_ref().map(|s| &s.address)
                                == Some(&sender_1.as_ref().to_vec().into()))
                                || (event.package_id.as_ref().map(|p| &p.object_id)
                                    == Some(&nft_package_id.as_ref().to_vec().into())),
                            "AnyEventFilter should receive events from both events: {:?}",
                            event.package_id.as_ref().map(|p| &p.object_id)
                        );

                        any_events.push(event.clone());
                    }
                }
                Err(e) => panic!("AnyEventFilter client error: {e}"),
            }
        }
    })
    .await;

    assert!(result.is_ok(), "AnyEventFilter should receive events");

    // - AnyEventFilter: receives all 3 events from both filters
    //  (2 from sender_1 and 1 NFT from sender_2)
    assert_eq!(
        any_events.len(),
        3,
        "AnyEventFilter should receive all 3 events"
    );
}
