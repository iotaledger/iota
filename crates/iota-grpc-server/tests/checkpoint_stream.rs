// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
mod common;
use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
    time::Duration,
};

use common::MockGrpcStateReader;
use iota_config::node::GrpcApiConfig;
use iota_grpc_client::{CheckpointStreamItem, Client};
use iota_grpc_server::GrpcServerHandle;
use iota_grpc_types::v1::{filter, ledger_service::checkpoint_data};
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::{
    base_types::{IotaAddress, random_object_ref},
    crypto::{AccountKeyPair, get_key_pair},
    effects::{TestEffectsBuilder, TransactionEvents},
    event::Event,
    full_checkpoint_content::{CheckpointData, CheckpointTransaction},
    messages_checkpoint::CheckpointSequenceNumber,
};
use move_core_types::{account_address::AccountAddress, ident_str, language_storage::StructTag};
use prost::Message;
use tokio_stream::StreamExt;

fn mock_checkpoint_data(sequence_number: u64) -> CheckpointData {
    CheckpointData {
        checkpoint_summary: common::mock_summary(
            sequence_number,
            &common::EMPTY_CHECKPOINT_CONTENTS,
        ),
        checkpoint_contents: common::EMPTY_CHECKPOINT_CONTENTS.clone(),
        transactions: vec![],
    }
}

/// Create checkpoint data with a transaction from a specific sender.
fn mock_checkpoint_data_with_sender(
    sequence_number: u64,
    sender: IotaAddress,
    key: &AccountKeyPair,
) -> CheckpointData {
    let gas = random_object_ref();
    let transaction = TestTransactionBuilder::new(sender, gas, 1000)
        .transfer(random_object_ref(), sender)
        .build_and_sign(key);
    let effects = TestEffectsBuilder::new(transaction.data()).build();
    CheckpointData {
        checkpoint_summary: common::mock_summary(
            sequence_number,
            &common::EMPTY_CHECKPOINT_CONTENTS,
        ),
        checkpoint_contents: common::EMPTY_CHECKPOINT_CONTENTS.clone(),
        transactions: vec![CheckpointTransaction {
            transaction,
            effects,
            events: None,
            input_objects: vec![],
            output_objects: vec![],
        }],
    }
}

fn build_large_checkpoint_transactions() -> Vec<CheckpointTransaction> {
    // Create many dummy transactions to exceed the message size limit when chunked.
    // Each transaction is roughly 500-1000 bytes when serialized as protobuf.
    let num_transactions = 50000;
    let mut transactions = Vec::with_capacity(num_transactions);

    for _ in 0..num_transactions {
        let (sender, key): (_, AccountKeyPair) = get_key_pair();
        let gas = random_object_ref();
        let transaction = TestTransactionBuilder::new(sender, gas, 1000)
            .transfer(random_object_ref(), sender)
            .build_and_sign(&key);

        let effects = TestEffectsBuilder::new(transaction.data()).build();

        transactions.push(CheckpointTransaction {
            transaction,
            effects,
            events: None,
            input_objects: vec![],  // Empty for simplicity
            output_objects: vec![], // Empty for simplicity
        });
    }

    transactions
}

/// Helper to set up test server with specific large checkpoints.
async fn test_server_and_client_setup_with_large_checkpoints<
    I: Iterator<Item = u64>,
    L: Iterator<Item = u64>,
>(
    checkpoint_range: I,
    large_checkpoints: L,
    config_customizer: impl FnOnce(&mut GrpcApiConfig),
    client_max_message_size_bytes: Option<u32>,
) -> (
    GrpcServerHandle,
    Client,
    Arc<Mutex<HashSet<CheckpointSequenceNumber>>>,
) {
    let mut mock = MockGrpcStateReader::new_from_iter(checkpoint_range);
    mock.large_checkpoint_transactions = build_large_checkpoint_transactions();
    let mock = Arc::new(mock);

    // Mark specified checkpoints as large
    for seq in large_checkpoints {
        mock.mark_checkpoint_as_large(seq);
    }

    test_server_and_client_setup(
        std::iter::empty(),
        config_customizer,
        Some(mock),
        client_max_message_size_bytes,
    )
    .await
}

/// Set up a test server and high-level `Client` with a set of available
/// checkpoint sequence numbers.
async fn test_server_and_client_setup<I: Iterator<Item = u64>>(
    checkpoint_range: I,
    config_customizer: impl FnOnce(&mut GrpcApiConfig),
    mock_state_reader: Option<Arc<MockGrpcStateReader>>,
    client_max_message_size_bytes: Option<u32>,
) -> (
    GrpcServerHandle,
    Client,
    Arc<Mutex<HashSet<CheckpointSequenceNumber>>>,
) {
    let mock = mock_state_reader
        .unwrap_or_else(|| Arc::new(MockGrpcStateReader::new_from_iter(checkpoint_range)));
    let checkpoints = mock.checkpoints.clone();

    let (server_handle, _) = common::start_test_server(mock, config_customizer).await;

    let server_addr = server_handle.address();
    let mut client = Client::connect(&format!("http://{server_addr}"))
        .await
        .expect("Failed to connect to gRPC server");

    if let Some(max_size) = client_max_message_size_bytes {
        client = client.with_max_decoding_message_size(usize::try_from(max_size).unwrap());
    }

    (server_handle, client, checkpoints)
}

// Helper function to spawn a background checkpoint sender for checkpoint data
fn spawn_checkpoint_sender(server_handle: &GrpcServerHandle, start_seq: u64) {
    let data_broadcaster = server_handle.checkpoint_data_broadcaster().clone();

    tokio::spawn(async move {
        let mut seq = start_seq;
        loop {
            let data = mock_checkpoint_data(seq);
            data_broadcaster.send_traced(&data);
            seq += 1;
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    });
}

#[tokio::test]
async fn test_start_sequence_number_only() {
    let (server_handle, client, _) = test_server_and_client_setup(0..=10, |_| {}, None, None).await;

    let range = (Some(5), None);

    let mut stream = client
        .stream_checkpoints(range.0, range.1, None, None, None)
        .await
        .unwrap();

    let mut results = Vec::new();

    // Start the checkpoint sender after we've subscribed to the stream
    spawn_checkpoint_sender(&server_handle, 11);

    tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(res) = stream.body_mut().next().await {
            match res {
                Ok(response) => {
                    let sequence_number = response.sequence_number();
                    // Only collect the expected range
                    if sequence_number > 30 {
                        break;
                    }
                    results.push(sequence_number);
                }
                Err(iota_grpc_client::Error::Grpc(status))
                    if status.code() == tonic::Code::NotFound =>
                {
                    break;
                }
                Err(e) => panic!("Unexpected error in stream: {e:?}"),
            }
        }
    })
    .await
    .expect("waiting for stream timed out");

    assert_eq!(results, (5..=30).collect::<Vec<_>>());

    // Clean up
    server_handle
        .shutdown()
        .await
        .expect("Failed to shutdown server");
}

#[tokio::test]
async fn test_start_and_future_end_sequence_number() {
    let (server_handle, client, _) = test_server_and_client_setup(0..=10, |_| {}, None, None).await;

    spawn_checkpoint_sender(&server_handle, 11);

    let range = (Some(3), Some(15));

    let mut stream = client
        .stream_checkpoints(range.0, range.1, None, None, None)
        .await
        .unwrap();

    let mut results = Vec::new();

    tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(res) = stream.body_mut().next().await {
            match res {
                Ok(response) => {
                    let sequence_number = response.sequence_number();
                    // Only collect the expected range
                    if sequence_number > 7 {
                        break;
                    }
                    results.push(sequence_number);
                }
                Err(iota_grpc_client::Error::Grpc(status))
                    if status.code() == tonic::Code::NotFound =>
                {
                    break;
                }
                Err(e) => panic!("Unexpected error in stream: {e:?}"),
            }
        }
    })
    .await
    .expect("waiting for stream timed out");

    assert_eq!(results, (3..=7).collect::<Vec<_>>());

    // Clean up
    server_handle
        .shutdown()
        .await
        .expect("Failed to shutdown server");
}

#[tokio::test]
async fn test_historical_end_sequence_number_only() {
    let (server_handle, client, _) = test_server_and_client_setup(0..=10, |_| {}, None, None).await;

    let range = (None, Some(4));

    let mut stream = client
        .stream_checkpoints(range.0, range.1, None, None, None)
        .await
        .unwrap();

    let mut results = Vec::new();

    tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(res) = stream.body_mut().next().await {
            match res {
                Ok(response) => {
                    let sequence_number = response.sequence_number();
                    results.push(sequence_number);
                }
                Err(iota_grpc_client::Error::Grpc(status))
                    if status.code() == tonic::Code::NotFound =>
                {
                    break;
                }
                Err(e) => panic!("Unexpected error in stream: {e:?}"),
            }
        }
    })
    .await
    .expect("waiting for stream timed out");

    assert_eq!(results, vec![4]);

    // Clean up
    server_handle
        .shutdown()
        .await
        .expect("Failed to shutdown server");
}

#[tokio::test]
async fn test_future_end_sequence_number_only_full() {
    let (server_handle, client, _) = test_server_and_client_setup(0..=10, |_| {}, None, None).await;
    spawn_checkpoint_sender(&server_handle, 11);

    let range = (None, Some(100));

    let mut stream = client
        .stream_checkpoints(range.0, range.1, None, None, None)
        .await
        .unwrap();

    let mut results = Vec::new();

    tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(res) = stream.body_mut().next().await {
            match res {
                Ok(response) => {
                    let sequence_number = response.sequence_number();
                    results.push(sequence_number);
                }
                Err(iota_grpc_client::Error::Grpc(status))
                    if status.code() == tonic::Code::NotFound =>
                {
                    break;
                }
                Err(e) => panic!("Unexpected error in stream: {e:?}"),
            }
        }
    })
    .await
    .expect("waiting for stream timed out");

    assert_eq!(results, vec![100]);

    // Clean up
    server_handle
        .shutdown()
        .await
        .expect("Failed to shutdown server");
}

#[tokio::test]
async fn test_both_indices_omitted() {
    let (server_handle, client, _) = test_server_and_client_setup(0..=10, |_| {}, None, None).await;

    // Subscribe to the stream after buffer is pre-filled (0..=10)
    let range = (None, None);

    let mut stream = client
        .stream_checkpoints(range.0, range.1, None, None, None)
        .await
        .unwrap();

    // Now send new checkpoints (live) after subscribing
    spawn_checkpoint_sender(&server_handle, 11);

    let mut results = Vec::new();

    // Collect enough checkpoints to see both buffered and live ones
    tokio::time::timeout(Duration::from_secs(10), async {
        let mut count = 0;

        while let Some(res) = stream.body_mut().next().await {
            match res {
                Ok(response) => {
                    let sequence_number = response.sequence_number();
                    results.push(sequence_number);
                    count += 1;
                    if count >= 15 {
                        break;
                    }
                }
                Err(iota_grpc_client::Error::Grpc(status))
                    if status.code() == tonic::Code::NotFound =>
                {
                    break;
                }
                Err(e) => panic!("Unexpected error in stream: {e:?}"),
            }
        }
    })
    .await
    .expect("waiting for stream timed out");

    // The first 11 should be 0..=10 (buffered), then live ones (11, 12, ...)
    assert_eq!(&results[..], &(10..=24).collect::<Vec<_>>()[..]);

    // Clean up
    server_handle
        .shutdown()
        .await
        .expect("Failed to shutdown server");
}

#[tokio::test]
async fn test_historical_to_live_gap_fill() {
    // Simulate storage with checkpoints 0..=149 (missing 150)
    let (server_handle, client, _) =
        test_server_and_client_setup(0..=149, |_| {}, None, None).await;

    // Client requests from 0 (historical) - should get 0..=149 from storage, then
    // 150 from broadcast
    let range = (Some(0), None);

    let mut stream = client
        .stream_checkpoints(range.0, range.1, None, None, None)
        .await
        .unwrap();

    // Simulate broadcast of checkpoint 150 AFTER subscribing
    let data_150 = mock_checkpoint_data(150);
    server_handle
        .checkpoint_data_broadcaster()
        .send_traced(&data_150);

    let mut results = Vec::new();

    // Collect up to 151 checkpoints
    tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(res) = stream.body_mut().next().await {
            match res {
                Ok(response) => {
                    let sequence_number = response.sequence_number();
                    results.push(sequence_number);
                    if sequence_number == 150 {
                        break;
                    }
                }
                Err(iota_grpc_client::Error::Grpc(status))
                    if status.code() == tonic::Code::NotFound =>
                {
                    break;
                }
                Err(e) => panic!("Unexpected error in stream: {e:?}"),
            }
        }
    })
    .await
    .expect("waiting for stream timed out");

    // Assert we got all checkpoints 0..=150 (0..=149 from storage, 150 from
    // broadcast)
    assert_eq!(results, (0..=150u64).collect::<Vec<_>>());

    // Clean up
    server_handle
        .shutdown()
        .await
        .expect("Failed to shutdown server");
}

#[tokio::test(flavor = "current_thread")]
async fn test_gap_fill_with_slow_client() {
    // Pre-populate storage with checkpoints 0..=10 before spawning the producer
    let (server_handle, client, checkpoints) = test_server_and_client_setup(
        0..=10,
        |config| {
            config.broadcast_buffer_size = 5;
        },
        None,
        None,
    )
    .await;

    // Producer: generates checkpoints 11..=200, one every 100ms
    tokio::spawn({
        let data_broadcaster = server_handle.checkpoint_data_broadcaster().clone();
        let checkpoints = checkpoints.clone();
        async move {
            for i in 11..=200u64 {
                let data = mock_checkpoint_data(i);
                checkpoints.lock().unwrap().insert(i);
                data_broadcaster.send_traced(&data);
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    });

    // Client: slow consumer
    let range = (Some(0), None);

    let mut stream = client
        .stream_checkpoints(range.0, range.1, None, None, None)
        .await
        .unwrap();

    let mut results = Vec::new();

    tokio::time::timeout(Duration::from_secs(120), async {
        while let Some(res) = stream.body_mut().next().await {
            match res {
                Ok(response) => {
                    let sequence_number = response.sequence_number();
                    results.push(sequence_number);
                    tokio::time::sleep(Duration::from_millis(250)).await; // slow down the client
                    if sequence_number >= 20 {
                        break;
                    }
                }
                Err(iota_grpc_client::Error::Grpc(status))
                    if status.code() == tonic::Code::NotFound =>
                {
                    break;
                }
                Err(e) => panic!("Unexpected error in stream: {e:?}"),
            }
        }
    })
    .await
    .expect("waiting for stream timed out");

    // Assert we got all checkpoints 0..=20
    assert_eq!(results, (0..=20u64).collect::<Vec<_>>());

    // Clean up
    server_handle
        .shutdown()
        .await
        .expect("Failed to shutdown server");
}

#[tokio::test]
async fn test_chunked_checkpoint_streaming() {
    // Test chunking by using a naturally large checkpoint that exceeds 4MB
    let (server_handle, client, _checkpoints) =
        test_server_and_client_setup_with_large_checkpoints(
            0..=1,
            std::iter::once(0), // Mark checkpoint 0 as large
            |config| {
                config.max_message_size_bytes = 4 * 1024 * 1024; // 4 MB
            },
            Some(4 * 1024 * 1024),
        )
        .await;

    // Test individual checkpoint retrieval
    let individual_checkpoint = client
        .get_checkpoint_by_sequence_number(0, None, None, None)
        .await
        .expect("get_checkpoint should work");

    // Verify the checkpoint data is correct
    assert_eq!(individual_checkpoint.body().sequence_number(), 0);
    let summary = individual_checkpoint
        .body()
        .summary()
        .expect("should have summary")
        .summary()
        .expect("should have checkpoint summary");
    assert_eq!(summary.epoch, 0);

    // Test streaming checkpoints - this should also work with small chunks
    let mut stream = client
        .stream_checkpoints(Some(0), Some(0), None, None, None)
        .await
        .unwrap();

    let streamed_checkpoint = stream
        .body_mut()
        .next()
        .await
        .expect("stream should have data")
        .expect("stream should not error");

    // Verify the streamed checkpoint data matches
    assert_eq!(streamed_checkpoint.sequence_number(), 0);
    let streamed_summary = streamed_checkpoint
        .summary()
        .expect("should have summary")
        .summary()
        .expect("should have checkpoint summary");
    assert_eq!(streamed_summary.epoch, 0);

    // Clean up
    server_handle
        .shutdown()
        .await
        .expect("Failed to shutdown server");
}

#[tokio::test]
async fn test_filter_checkpoints_validation() {
    let (server_handle, client, _) = test_server_and_client_setup(0..=5, |_| {}, None, None).await;

    // filter_checkpoints=true with no filters should fail
    let result = client
        .stream_checkpoints_filtered(Some(0), Some(5), None, None, None, None)
        .await;
    assert!(result.is_err(), "expected error when no filters are set");

    // tx filter without transactions in read_mask should fail
    let (sender, _): (IotaAddress, AccountKeyPair) = get_key_pair();
    let sender_bytes = sender.to_inner();
    let tx_filter = filter::TransactionFilter::default().with_sender(
        filter::AddressFilter::default().with_address(
            iota_grpc_types::v1::types::Address::default().with_address(sender_bytes.to_vec()),
        ),
    );

    let result = client
        .stream_checkpoints_filtered(
            Some(0),
            Some(5),
            Some("checkpoint"),
            Some(tx_filter),
            None,
            None,
        )
        .await;
    assert!(
        result.is_err(),
        "expected error when tx filter is set but transactions not in read_mask"
    );

    server_handle
        .shutdown()
        .await
        .expect("Failed to shutdown server");
}

#[tokio::test]
async fn test_filter_checkpoints_streaming() {
    let (server_handle, client, _) = test_server_and_client_setup(0..=0, |_| {}, None, None).await;

    let (sender, key): (IotaAddress, AccountKeyPair) = get_key_pair();
    let sender_bytes = sender.to_inner();

    // Create a sender filter matching our known sender
    let make_tx_filter = || {
        iota_grpc_types::v1::filter::TransactionFilter::default().with_sender(
            iota_grpc_types::v1::filter::AddressFilter::default().with_address(
                iota_grpc_types::v1::types::Address::default().with_address(sender_bytes.to_vec()),
            ),
        )
    };

    // Scenario 1: matching txs are returned, non-matching are skipped
    let mut stream = client
        .stream_checkpoints_filtered(
            None,
            None,
            Some("checkpoint,transactions"),
            Some(make_tx_filter()),
            None,
            None,
        )
        .await
        .unwrap();

    // Broadcast checkpoint 1 with matching sender
    server_handle
        .checkpoint_data_broadcaster()
        .send_traced(&mock_checkpoint_data_with_sender(1, sender, &key));
    // Broadcast checkpoint 2 with no transactions (should be skipped)
    server_handle
        .checkpoint_data_broadcaster()
        .send_traced(&mock_checkpoint_data(2));
    // Broadcast checkpoint 3 with matching sender
    server_handle
        .checkpoint_data_broadcaster()
        .send_traced(&mock_checkpoint_data_with_sender(3, sender, &key));

    let mut results = Vec::new();
    tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(res) = stream.body_mut().next().await {
            match res {
                Ok(CheckpointStreamItem::Checkpoint(response)) => {
                    results.push(response.sequence_number());
                    if results.len() >= 2 {
                        break;
                    }
                }
                Ok(_) => {}
                Err(e) => panic!("Unexpected error: {e:?}"),
            }
        }
    })
    .await
    .expect("waiting for stream timed out");
    // Only checkpoints 1 and 3 should be received (checkpoint 2 was filtered out)
    assert_eq!(results, vec![1, 3]);
    drop(stream);

    // Scenario 2: non-matching checkpoints are skipped until a match
    let mut stream = client
        .stream_checkpoints_filtered(
            None,
            None,
            Some("checkpoint,transactions"),
            Some(make_tx_filter()),
            None,
            None,
        )
        .await
        .unwrap();

    // Broadcast checkpoints with no transactions (should all be skipped)
    for i in 1..=5 {
        server_handle
            .checkpoint_data_broadcaster()
            .send_traced(&mock_checkpoint_data(i));
    }
    // Broadcast checkpoint with a different sender (should be skipped)
    let (other_sender, other_key): (IotaAddress, AccountKeyPair) = get_key_pair();
    server_handle
        .checkpoint_data_broadcaster()
        .send_traced(&mock_checkpoint_data_with_sender(
            6,
            other_sender,
            &other_key,
        ));
    // Finally broadcast one with the matching sender
    server_handle
        .checkpoint_data_broadcaster()
        .send_traced(&mock_checkpoint_data_with_sender(7, sender, &key));

    let mut results = Vec::new();
    tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(res) = stream.body_mut().next().await {
            match res {
                Ok(CheckpointStreamItem::Checkpoint(response)) => {
                    results.push(response.sequence_number());
                    return; // Just collect the first match
                }
                Ok(_) => {}
                Err(e) => panic!("Unexpected error: {e:?}"),
            }
        }
    })
    .await
    .expect("waiting for stream timed out");
    // Only checkpoint 7 should be received (all others were filtered out)
    assert_eq!(results, vec![7]);

    server_handle
        .shutdown()
        .await
        .expect("Failed to shutdown server");
}

#[tokio::test]
async fn test_get_checkpoint_pruned_returns_not_found() {
    // Set up mock with checkpoints 0..=10 but lowest_available_checkpoint = 5
    let mock =
        Arc::new(MockGrpcStateReader::new_from_iter(0..=10).with_lowest_available_checkpoint(5));

    let (server_handle, client, _) =
        test_server_and_client_setup(std::iter::empty(), |_| {}, Some(mock), None).await;

    // Requesting checkpoint 0 (genesis, still in DB) should fail because it's below
    // lowest_available_checkpoint
    let result = client
        .get_checkpoint_by_sequence_number(0, None, None, None)
        .await;
    assert!(result.is_err(), "Expected error for pruned checkpoint");
    match result.unwrap_err() {
        iota_grpc_client::Error::Grpc(status) => {
            assert_eq!(status.code(), tonic::Code::NotFound);
            assert!(
                status
                    .message()
                    .contains("below the lowest available checkpoint"),
                "Error message should mention lowest available checkpoint: {}",
                status.message()
            );
        }
        e => panic!("Expected Grpc error, got: {e:?}"),
    }

    // Requesting checkpoint 5 (at lowest_available) should succeed
    let result = client
        .get_checkpoint_by_sequence_number(5, None, None, None)
        .await;
    assert!(result.is_ok(), "Checkpoint at lowest_available should work");

    server_handle
        .shutdown()
        .await
        .expect("Failed to shutdown server");
}

#[tokio::test]
async fn test_stream_checkpoint_pruned_start_returns_not_found() {
    // Set up mock with checkpoints 0..=10 but lowest_available_checkpoint = 5
    let mock =
        Arc::new(MockGrpcStateReader::new_from_iter(0..=10).with_lowest_available_checkpoint(5));

    let (server_handle, client, _) =
        test_server_and_client_setup(std::iter::empty(), |_| {}, Some(mock), None).await;

    // Streaming from checkpoint 0 should fail because it's below
    // lowest_available_checkpoint. The error surfaces at the RPC level
    // since the pruning check happens before the stream is created.
    let result = client
        .stream_checkpoints(Some(0), Some(10), None, None, None)
        .await;

    match result {
        Err(iota_grpc_client::Error::Grpc(status)) => {
            assert_eq!(status.code(), tonic::Code::NotFound);
            assert!(
                status
                    .message()
                    .contains("below the lowest available checkpoint"),
                "Error message should mention lowest available checkpoint: {}",
                status.message()
            );
        }
        Err(e) => panic!("Expected Grpc error, got: {e:?}"),
        Ok(_) => panic!("Expected error for pruned start checkpoint"),
    }

    server_handle
        .shutdown()
        .await
        .expect("Failed to shutdown server");
}

/// Build checkpoint transactions, each optionally carrying `events_per_tx`
/// events.
fn build_checkpoint_transactions_with_events(
    count: usize,
    events_per_tx: usize,
) -> Vec<CheckpointTransaction> {
    let mut transactions = Vec::with_capacity(count);
    for _ in 0..count {
        let (sender, key): (_, AccountKeyPair) = get_key_pair();
        let gas = random_object_ref();
        let transaction = TestTransactionBuilder::new(sender, gas, 1000)
            .transfer(random_object_ref(), sender)
            .build_and_sign(&key);
        let effects = TestEffectsBuilder::new(transaction.data()).build();
        let events = if events_per_tx > 0 {
            let mut data = Vec::with_capacity(events_per_tx);
            for _ in 0..events_per_tx {
                data.push(Event::new(
                    &AccountAddress::ZERO,
                    ident_str!("test_module"),
                    sender,
                    StructTag {
                        address: AccountAddress::ZERO,
                        module: ident_str!("test_module").into(),
                        name: ident_str!("TestEvent").into(),
                        type_params: vec![],
                    },
                    vec![0u8; 64], // 64 bytes of dummy content
                ));
            }
            Some(TransactionEvents { data })
        } else {
            None
        };
        transactions.push(CheckpointTransaction {
            transaction,
            effects,
            events,
            input_objects: vec![],
            output_objects: vec![],
        });
    }
    transactions
}

/// Collect all CheckpointData messages from a tonic streaming response,
/// partitioning payload sizes by type.
async fn collect_checkpoint_data_stream(
    mut stream: tonic::codec::Streaming<iota_grpc_types::v1::ledger_service::CheckpointData>,
) -> (
    Vec<iota_grpc_types::v1::ledger_service::CheckpointData>,
    Vec<usize>,
    Vec<usize>,
) {
    let mut all_messages = Vec::new();
    let mut tx_batch_sizes = Vec::new();
    let mut event_batch_sizes = Vec::new();

    while let Some(msg) = stream.message().await.expect("stream should not error") {
        match &msg.payload {
            Some(checkpoint_data::Payload::ExecutedTransactions(_)) => {
                tx_batch_sizes.push(msg.encoded_len());
            }
            Some(checkpoint_data::Payload::Events(_)) => {
                event_batch_sizes.push(msg.encoded_len());
            }
            _ => {}
        }
        all_messages.push(msg);
    }

    (all_messages, tx_batch_sizes, event_batch_sizes)
}

/// Issue a `GetCheckpoint` request via the raw tonic client.
async fn get_checkpoint_raw(
    client: &mut iota_grpc_types::v1::ledger_service::ledger_service_client::LedgerServiceClient<
        tonic::transport::Channel,
    >,
    read_mask: &str,
    max_message_size: u32,
) -> tonic::codec::Streaming<iota_grpc_types::v1::ledger_service::CheckpointData> {
    use iota_grpc_types::{field::FieldMaskUtil, v1::ledger_service::GetCheckpointRequest};

    let req = GetCheckpointRequest::default()
        .with_sequence_number(0)
        .with_read_mask(prost_types::FieldMask::from_str(read_mask))
        .with_max_message_size_bytes(max_message_size);

    client
        .get_checkpoint(req)
        .await
        .expect("get_checkpoint should succeed")
        .into_inner()
}

#[tokio::test]
async fn test_chunked_checkpoint_message_sizes_within_limit() {
    // 10 000 transactions → total payload exceeds the 4 MB minimum message size
    // enforced by the server, which enables the splitting test.
    let transactions = build_checkpoint_transactions_with_events(10_000, 0);
    let summary = common::mock_summary(0, &common::EMPTY_CHECKPOINT_CONTENTS);
    let contents = common::EMPTY_CHECKPOINT_CONTENTS.clone();

    let state_reader = Arc::new(MockGrpcStateReader {
        summary: Some(summary),
        contents: Some(contents),
        checkpoint_transactions: transactions,
        ..Default::default()
    });
    let (server_handle, _) = common::start_test_server(state_reader, |config| {
        // Server max = 128 MB so the unlimited pass fits in one batch.
        config.max_message_size_bytes = 128 * 1024 * 1024;
    })
    .await;
    let addr = server_handle.address();

    // Use the raw tonic client instead of the high-level Client so we can
    // inspect individual streamed CheckpointData messages and verify their
    // sizes. The high-level client reassembles them into a single response.
    let channel = tonic::transport::Channel::from_shared(format!("http://{addr}"))
        .unwrap()
        .connect()
        .await
        .expect("connect");
    let mut client =
        iota_grpc_types::v1::ledger_service::ledger_service_client::LedgerServiceClient::new(
            channel,
        )
        .max_decoding_message_size(128 * 1024 * 1024);

    let read_mask = "checkpoint.summary,transactions";

    // --- Pass 1: unlimited (128 MB) → measure single-batch encoded size ---
    let stream = get_checkpoint_raw(&mut client, read_mask, 128 * 1024 * 1024).await;
    let (_, tx_sizes_unlimited, _) = collect_checkpoint_data_stream(stream).await;
    assert_eq!(
        tx_sizes_unlimited.len(),
        1,
        "With 128 MB limit all transactions should fit in a single batch"
    );
    let exact_batch_size = u32::try_from(tx_sizes_unlimited[0]).unwrap();
    assert!(
        exact_batch_size >= 4 * 1024 * 1024,
        "Test prerequisite: single batch ({exact_batch_size}) must be >= 4 MB \
         so the server does not clamp the limit"
    );

    // --- Pass 2: exact limit → should still fit in one batch ---
    let stream = get_checkpoint_raw(&mut client, read_mask, exact_batch_size).await;
    let (_, tx_sizes_exact, _) = collect_checkpoint_data_stream(stream).await;
    assert_eq!(
        tx_sizes_exact.len(),
        1,
        "At exact limit ({exact_batch_size}) all transactions should still fit in one batch"
    );

    // --- Pass 3: exact - 1 → must split ---
    let tight_limit = exact_batch_size - 1;
    let stream = get_checkpoint_raw(&mut client, read_mask, tight_limit).await;
    let (all_messages, tx_sizes_split, _) = collect_checkpoint_data_stream(stream).await;
    assert!(
        tx_sizes_split.len() > 1,
        "At limit {tight_limit} (exact-1) transactions must be split, got {} batch(es)",
        tx_sizes_split.len()
    );

    // Every message must be within the limit.
    for (i, msg) in all_messages.iter().enumerate() {
        let size = msg.encoded_len();
        assert!(
            size <= usize::try_from(tight_limit).unwrap(),
            "Message {i} has encoded_len {size} which exceeds limit {tight_limit}"
        );
    }

    server_handle.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn test_chunked_checkpoint_event_message_sizes_within_limit() {
    // 2 000 transactions × 5 events each → total event payload exceeds 4 MB.
    let transactions = build_checkpoint_transactions_with_events(2_000, 5);
    let summary = common::mock_summary(0, &common::EMPTY_CHECKPOINT_CONTENTS);
    let contents = common::EMPTY_CHECKPOINT_CONTENTS.clone();

    let state_reader = Arc::new(MockGrpcStateReader {
        summary: Some(summary),
        contents: Some(contents),
        checkpoint_transactions: transactions,
        ..Default::default()
    });
    let (server_handle, _) = common::start_test_server(state_reader, |config| {
        config.max_message_size_bytes = 128 * 1024 * 1024;
    })
    .await;
    let addr = server_handle.address();

    // Use the raw tonic client instead of the high-level Client so we can
    // inspect individual streamed CheckpointData messages and verify their
    // sizes. The high-level client reassembles them into a single response.
    let channel = tonic::transport::Channel::from_shared(format!("http://{addr}"))
        .unwrap()
        .connect()
        .await
        .expect("connect");
    let mut client =
        iota_grpc_types::v1::ledger_service::ledger_service_client::LedgerServiceClient::new(
            channel,
        )
        .max_decoding_message_size(128 * 1024 * 1024);

    let read_mask = "checkpoint.summary,events";

    // --- Pass 1: unlimited (128 MB) → measure single-batch encoded size ---
    let stream = get_checkpoint_raw(&mut client, read_mask, 128 * 1024 * 1024).await;
    let (_, _, event_sizes_unlimited) = collect_checkpoint_data_stream(stream).await;
    assert_eq!(
        event_sizes_unlimited.len(),
        1,
        "With 128 MB limit all events should fit in a single batch"
    );
    let exact_batch_size = u32::try_from(event_sizes_unlimited[0]).unwrap();
    assert!(
        exact_batch_size >= 4 * 1024 * 1024,
        "Test prerequisite: single batch ({exact_batch_size}) must be >= 4 MB \
         so the server does not clamp the limit"
    );

    // --- Pass 2: exact limit → should still fit in one batch ---
    let stream = get_checkpoint_raw(&mut client, read_mask, exact_batch_size).await;
    let (_, _, event_sizes_exact) = collect_checkpoint_data_stream(stream).await;
    assert_eq!(
        event_sizes_exact.len(),
        1,
        "At exact limit ({exact_batch_size}) all events should still fit in one batch"
    );

    // --- Pass 3: exact - 1 → must split ---
    let tight_limit = exact_batch_size - 1;
    let stream = get_checkpoint_raw(&mut client, read_mask, tight_limit).await;
    let (all_messages, _, event_sizes_split) = collect_checkpoint_data_stream(stream).await;
    assert!(
        event_sizes_split.len() > 1,
        "At limit {tight_limit} (exact-1) events must be split, got {} batch(es)",
        event_sizes_split.len()
    );

    // Every message must be within the limit.
    for (i, msg) in all_messages.iter().enumerate() {
        let size = msg.encoded_len();
        assert!(
            size <= usize::try_from(tight_limit).unwrap(),
            "Message {i} has encoded_len {size} which exceeds limit {tight_limit}"
        );
    }

    server_handle.shutdown().await.expect("shutdown");
}
