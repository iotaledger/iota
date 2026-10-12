// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use futures::StreamExt;
use iota_grpc_types::{
    field::FieldMaskUtil,
    v1::ledger_service::{
        GetCheckpointRequest, GetTransactionsRequest, TransactionRequest, TransactionRequests,
        ledger_service_client::LedgerServiceClient,
    },
};
use prost::Message;

use crate::{
    common::{self, MockGrpcStateReader},
    harness::{
        ListFixture, ListResponses, OBJECTS, call_get_objects, call_list_methods, connect,
        list_fixture, start, start_with,
    },
};

#[tokio::test]
async fn item_sizes_of_get_objects_match_what_the_client_receives() {
    let call = call_get_objects().await;
    let metrics = &call.reader;

    let response_item_sizes: Vec<usize> = call
        .responses
        .iter()
        .flat_map(|m| m.objects.iter().map(|o| o.encoded_len()))
        .collect();
    assert_eq!(response_item_sizes.len(), OBJECTS);
    assert_eq!(
        metrics.value("response_item_bytes_peak", &[("kind", "object")]) as usize,
        *response_item_sizes.iter().max().unwrap()
    );
}

#[tokio::test]
async fn a_checkpoint_is_observed_as_a_whole() {
    let (handle, metrics) = start(MockGrpcStateReader::new_from_iter(0..=10)).await;
    let mut ledger = LedgerServiceClient::new(connect(&handle).await);

    let messages: Vec<_> = ledger
        .get_checkpoint(GetCheckpointRequest::default().with_sequence_number(5))
        .await
        .unwrap()
        .into_inner()
        .map(|message| message.unwrap())
        .collect()
        .await;
    assert!(messages.len() >= 2);

    let total: usize = messages.iter().map(|m| m.encoded_len()).sum();
    assert_eq!(
        metrics.value("response_item_bytes_peak", &[("kind", "checkpoint")]) as usize,
        total
    );
}

#[tokio::test]
async fn transactions_and_events_of_a_checkpoint_are_observed_as_items() {
    use iota_grpc_types::v1::ledger_service::checkpoint_data::Payload;

    let mut mock = MockGrpcStateReader::new_from_iter(0..=10);
    mock.checkpoint_transactions = common::build_checkpoint_transactions_with_events(4, 3);
    let (handle, metrics) = start(mock).await;
    let mut ledger = LedgerServiceClient::new(connect(&handle).await);

    let messages: Vec<_> = ledger
        .get_checkpoint(
            GetCheckpointRequest::default()
                .with_sequence_number(5)
                .with_read_mask(prost_types::FieldMask::from_str(
                    "checkpoint,transactions,events",
                )),
        )
        .await
        .unwrap()
        .into_inner()
        .map(|message| message.unwrap())
        .collect()
        .await;

    let mut transactions = Vec::new();
    let mut events = Vec::new();
    for message in &messages {
        match &message.payload {
            Some(Payload::ExecutedTransactions(batch)) => {
                transactions.extend(batch.executed_transactions.iter().map(|t| t.encoded_len()))
            }
            Some(Payload::Events(batch)) => {
                events.extend(batch.events.iter().map(|e| e.encoded_len()))
            }
            _ => {}
        }
    }
    assert_eq!((transactions.len(), events.len()), (4, 12));
    assert_eq!(
        metrics.value("response_item_bytes_peak", &[("kind", "transaction")]) as usize,
        *transactions.iter().max().unwrap()
    );
    assert_eq!(
        metrics.value("response_item_bytes_peak", &[("kind", "event")]) as usize,
        *events.iter().max().unwrap()
    );
}

#[tokio::test]
async fn items_of_the_list_calls_are_observed() {
    let ListFixture {
        mock,
        owner,
        parent,
        package,
    } = list_fixture();
    let (handle, metrics) = start(mock).await;
    let channel = connect(&handle).await;

    let ListResponses {
        owned,
        fields,
        versions,
    } = call_list_methods(channel, owner, parent, package).await;

    let check = |kind: &str, sizes: Vec<usize>| {
        assert!(!sizes.is_empty(), "{kind}: no items");
        assert_eq!(
            metrics.value("response_item_bytes_peak", &[("kind", kind)]) as usize,
            *sizes.iter().max().unwrap(),
            "{kind}"
        );
    };
    check(
        "owned_object",
        owned.objects.iter().map(|o| o.encoded_len()).collect(),
    );
    check(
        "dynamic_field",
        fields
            .dynamic_fields
            .iter()
            .map(|f| f.encoded_len())
            .collect(),
    );
    assert_eq!(owned.objects.len(), 3);
    assert_eq!(fields.dynamic_fields.len(), 4);
    assert_eq!(versions.versions.len(), 5);
}

#[tokio::test]
async fn item_sizes_of_get_transactions_match_what_the_client_receives() {
    let mut mock = MockGrpcStateReader::default();
    let mut digests = Vec::new();
    for _ in 0..5 {
        let (digest, transaction, effects) = common::create_test_transaction();
        digests.push(digest);
        mock.transactions.insert(digest, transaction);
        mock.effects.insert(digest, effects);
    }
    let (handle, metrics) =
        start_with(mock, |config| config.max_get_transactions_batch_size = 10).await;
    let mut ledger = LedgerServiceClient::new(connect(&handle).await);

    let request = GetTransactionsRequest::default()
        .with_requests(
            TransactionRequests::default().with_requests(
                digests
                    .iter()
                    .map(|digest| {
                        TransactionRequest::default().with_digest(
                            iota_grpc_types::v1::types::Digest::default()
                                .with_digest(digest.into_bytes().to_vec()),
                        )
                    })
                    .collect(),
            ),
        )
        .with_read_mask(prost_types::FieldMask::from_str(
            "transaction,signatures,effects",
        ));
    let responses: Vec<_> = ledger
        .get_transactions(request)
        .await
        .unwrap()
        .into_inner()
        .map(|response| response.unwrap())
        .collect()
        .await;

    let sizes: Vec<usize> = responses
        .iter()
        .flat_map(|response| response.transaction_results.iter().map(|t| t.encoded_len()))
        .collect();
    assert_eq!(sizes.len(), 5);
    assert_eq!(
        metrics.value("response_item_bytes_peak", &[("kind", "transaction")]) as usize,
        *sizes.iter().max().unwrap()
    );
}

#[tokio::test]
async fn a_checkpoint_of_many_messages_is_observed_as_the_sum_of_them() {
    use iota_grpc_types::v1::ledger_service::checkpoint_data::Payload;

    let mut mock = MockGrpcStateReader::new_from_iter(0..=10);
    mock.checkpoint_transactions = common::build_checkpoint_transactions_with_events(2500, 1);
    let (handle, metrics) = start(mock).await;
    let mut ledger = LedgerServiceClient::new(connect(&handle).await);

    let messages: Vec<_> = ledger
        .get_checkpoint(
            GetCheckpointRequest::default()
                .with_sequence_number(5)
                .with_read_mask(prost_types::FieldMask::from_str(
                    "checkpoint,transactions,events",
                ))
                .with_max_message_size_bytes(1024 * 1024),
        )
        .await
        .unwrap()
        .into_inner()
        .map(|message| message.unwrap())
        .collect()
        .await;

    let batches = messages
        .iter()
        .filter(|m| {
            matches!(
                m.payload,
                Some(Payload::ExecutedTransactions(_) | Payload::Events(_))
            )
        })
        .count();
    assert!(batches >= 3, "{batches} batches: the test needs splitting");
    let total: usize = messages.iter().map(|m| m.encoded_len()).sum();
    assert_eq!(
        metrics.value("response_item_bytes_peak", &[("kind", "checkpoint")]) as usize,
        total
    );
}

/// The sum of the encoded sizes of the messages of each checkpoint, in order.
/// A checkpoint ends with its end marker.
fn checkpoint_sums(messages: &[iota_grpc_types::v1::ledger_service::CheckpointData]) -> Vec<usize> {
    use iota_grpc_types::v1::ledger_service::checkpoint_data::Payload;

    let mut sums = vec![0];
    for message in messages {
        *sums.last_mut().unwrap() += message.encoded_len();
        if matches!(message.payload, Some(Payload::EndMarker(_))) {
            sums.push(0);
        }
    }
    sums.pop();
    sums
}

#[tokio::test]
async fn a_historical_stream_observes_each_checkpoint() {
    use iota_grpc_types::v1::ledger_service::StreamCheckpointsRequest;

    let mut mock = MockGrpcStateReader::new_from_iter(0..=10);
    mock.checkpoint_transactions = common::build_checkpoint_transactions_with_events(3, 2);
    let (handle, metrics) = start(mock).await;
    let mut ledger = LedgerServiceClient::new(connect(&handle).await);

    let messages: Vec<_> = ledger
        .stream_checkpoints(
            StreamCheckpointsRequest::default()
                .with_start_sequence_number(5)
                .with_end_sequence_number(7)
                .with_read_mask(prost_types::FieldMask::from_str(
                    "checkpoint,transactions,events",
                )),
        )
        .await
        .unwrap()
        .into_inner()
        .map(|message| message.unwrap())
        .collect()
        .await;

    let sums = checkpoint_sums(&messages);
    assert_eq!(sums.len(), 3);
    assert_eq!(
        metrics.value("response_item_bytes_peak", &[("kind", "checkpoint")]) as usize,
        *sums.iter().max().unwrap()
    );
    assert!(metrics.value("response_item_bytes_peak", &[("kind", "transaction")]) > 0.0);
}

#[tokio::test]
async fn a_live_stream_observes_each_checkpoint() {
    use iota_grpc_types::v1::ledger_service::StreamCheckpointsRequest;
    use iota_types::full_checkpoint_content::CheckpointData;

    let (handle, metrics) = start(MockGrpcStateReader::new_from_iter(0..=10)).await;
    let mut ledger = LedgerServiceClient::new(connect(&handle).await);
    let mut stream = ledger
        .stream_checkpoints(StreamCheckpointsRequest::default().with_start_sequence_number(11))
        .await
        .unwrap()
        .into_inner();

    let broadcaster = handle.checkpoint_data_broadcaster().clone();
    let mut messages = Vec::new();
    for sequence_number in 11..13 {
        broadcaster.send_traced(&CheckpointData {
            checkpoint_summary: common::mock_summary(
                sequence_number,
                &common::EMPTY_CHECKPOINT_CONTENTS,
            ),
            checkpoint_contents: common::EMPTY_CHECKPOINT_CONTENTS.clone(),
            transactions: vec![],
        });
        loop {
            let message = stream.next().await.unwrap().unwrap();
            let end = matches!(
                message.payload,
                Some(iota_grpc_types::v1::ledger_service::checkpoint_data::Payload::EndMarker(_))
            );
            messages.push(message);
            if end {
                break;
            }
        }
    }

    let sums = checkpoint_sums(&messages);
    assert_eq!(sums.len(), 2);
    assert_eq!(
        metrics.value("response_item_bytes_peak", &[("kind", "checkpoint")]) as usize,
        *sums.iter().max().unwrap()
    );
}
