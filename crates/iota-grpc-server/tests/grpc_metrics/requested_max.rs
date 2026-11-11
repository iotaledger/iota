// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_types::v1::ledger_service::{
    GetCheckpointRequest, StreamCheckpointsRequest, ledger_service_client::LedgerServiceClient,
};

use crate::{
    common::MockGrpcStateReader,
    harness::{MetricsReader, call_get_objects, connect, start},
};

#[tokio::test]
async fn the_requested_max_message_size_is_observed_and_a_missing_one_is_not() {
    let mut call = call_get_objects().await;
    let requested = call
        .reader
        .histogram_totals("requested_max_message_bytes", &[]);
    assert_eq!((requested.count, requested.sum), (1, 1024.0 * 1024.0));

    call.call_without_max_message_size().await;
    assert_eq!(
        call.reader
            .histogram_totals("requested_max_message_bytes", &[])
            .count,
        1
    );
}

#[tokio::test]
async fn the_checkpoint_calls_observe_the_requested_max_message_size() {
    let (handle, metrics) = start(MockGrpcStateReader::new_from_iter(0..=10)).await;
    let mut ledger = LedgerServiceClient::new(connect(&handle).await);
    let requested =
        |metrics: &MetricsReader| metrics.histogram_totals("requested_max_message_bytes", &[]);

    ledger
        .get_checkpoint(
            GetCheckpointRequest::default()
                .with_sequence_number(5)
                .with_max_message_size_bytes(2 * 1024 * 1024),
        )
        .await
        .unwrap();
    assert_eq!(
        (requested(&metrics).count, requested(&metrics).sum),
        (1, 2.0 * 1024.0 * 1024.0)
    );

    ledger
        .stream_checkpoints(
            StreamCheckpointsRequest::default().with_max_message_size_bytes(3 * 1024 * 1024),
        )
        .await
        .unwrap();
    assert_eq!(
        (requested(&metrics).count, requested(&metrics).sum),
        (2, 5.0 * 1024.0 * 1024.0)
    );
}

#[tokio::test]
async fn get_transactions_and_the_list_calls_observe_the_requested_max_message_size() {
    use iota_grpc_types::v1::{
        ledger_service::{GetTransactionsRequest, TransactionRequest, TransactionRequests},
        move_package_service::{
            ListPackageVersionsRequest, move_package_service_client::MovePackageServiceClient,
        },
        state_service::{
            ListDynamicFieldsRequest, ListOwnedObjectsRequest,
            state_service_client::StateServiceClient,
        },
    };

    use crate::{
        common,
        harness::{ListFixture, list_fixture},
    };

    const MIB: u32 = 1024 * 1024;
    let ListFixture {
        mock,
        owner,
        parent,
        package,
    } = list_fixture();
    let (handle, metrics) = start(mock).await;
    let channel = connect(&handle).await;
    let requested = |metrics: &MetricsReader| {
        let hist = metrics.histogram_totals("requested_max_message_bytes", &[]);
        (hist.count, hist.sum as u32)
    };

    StateServiceClient::new(channel.clone())
        .list_owned_objects(
            ListOwnedObjectsRequest::default()
                .with_owner(common::owner_proto(owner))
                .with_max_message_size_bytes(MIB),
        )
        .await
        .unwrap();
    assert_eq!(requested(&metrics), (1, MIB));

    StateServiceClient::new(channel.clone())
        .list_dynamic_fields(
            ListDynamicFieldsRequest::default()
                .with_parent(common::object_id_proto(parent))
                .with_max_message_size_bytes(2 * MIB),
        )
        .await
        .unwrap();
    assert_eq!(requested(&metrics), (2, 3 * MIB));

    MovePackageServiceClient::new(channel.clone())
        .list_package_versions(
            ListPackageVersionsRequest::default()
                .with_package_id(common::object_id_proto(package))
                .with_max_message_size_bytes(3 * MIB),
        )
        .await
        .unwrap();
    assert_eq!(requested(&metrics), (3, 6 * MIB));

    let mut ledger = LedgerServiceClient::new(channel);
    ledger
        .get_transactions(
            GetTransactionsRequest::default()
                .with_requests(TransactionRequests::default().with_requests(vec![
                    TransactionRequest::default().with_digest(
                        iota_grpc_types::v1::types::Digest::default().with_digest(vec![7u8; 32]),
                    ),
                ]))
                .with_max_message_size_bytes(4 * MIB),
        )
        .await
        .unwrap();
    assert_eq!(requested(&metrics), (4, 10 * MIB));
}
