// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_types::{
    headers,
    v1::ledger_service::{
        GetCheckpointRequest, GetEpochRequest, GetObjectsRequest, GetServiceInfoRequest,
        GetTransactionsRequest, StreamCheckpointsRequest,
    },
};
use iota_macros::sim_test;

use crate::{
    utils::setup_grpc_test_with_builder,
    v1::header::{parse_u64_header, verify_iota_headers},
};

#[sim_test]
async fn test_response_headers() {
    let (test_cluster, client) =
        setup_grpc_test_with_builder(|builder| builder.with_epoch_duration_ms(2000), None, None)
            .await;

    let mut ledger_client = client.ledger_service_client();

    // Test get_service_info
    {
        test_cluster.wait_for_checkpoint(2, None).await;

        let request = GetServiceInfoRequest::default();

        let response = ledger_client
            .get_service_info(request)
            .await
            .expect("gRPC call to get_service_info");

        let metadata = response.metadata();

        // Verify all required headers are present
        verify_iota_headers(metadata, "get_service_info");

        // Verify checkpoint_height value
        let checkpoint_height = parse_u64_header(metadata, headers::X_IOTA_CHECKPOINT_HEIGHT);
        assert!(
            checkpoint_height >= 2,
            "checkpoint_height should be at least 2, got {checkpoint_height}",
        );

        // Verify epoch value
        let epoch = parse_u64_header(metadata, headers::X_IOTA_EPOCH);
        assert_eq!(epoch, 0, "epoch should be 0, got {epoch}");
    }

    // Test get_epoch
    {
        test_cluster.wait_for_epoch(Some(2)).await;

        let request = GetEpochRequest::default().with_epoch(1);
        let response = ledger_client
            .get_epoch(request)
            .await
            .expect("gRPC call to get_epoch");

        let metadata = response.metadata();

        // Verify all required headers are present
        verify_iota_headers(metadata, "get_epoch");

        // Verify epoch value
        let epoch = parse_u64_header(metadata, headers::X_IOTA_EPOCH);
        assert!(epoch >= 1, "epoch should be at least 1, got {epoch}");
    }

    // Test get_objects
    {
        let request = GetObjectsRequest::default();

        let stream = ledger_client
            .get_objects(request)
            .await
            .expect("gRPC call to get_objects");

        // Get metadata from the first response
        let metadata = stream.metadata();
        verify_iota_headers(metadata, "get_objects");

        // Verify epoch value
        let epoch = parse_u64_header(metadata, headers::X_IOTA_EPOCH);
        assert!(epoch >= 1, "epoch should be at least 1, got {epoch}");
    }

    // Test get_transactions
    {
        let request = GetTransactionsRequest::default();

        let stream = ledger_client
            .get_transactions(request)
            .await
            .expect("gRPC call to get_transactions");

        // Get metadata from the response
        let metadata = stream.metadata();
        verify_iota_headers(metadata, "get_transactions");

        // Verify epoch value
        let epoch = parse_u64_header(metadata, headers::X_IOTA_EPOCH);
        assert!(epoch >= 1, "epoch should be at least 1, got {epoch}");
    }

    // Test get_checkpoint
    {
        let request = GetCheckpointRequest::default().with_latest(true);

        let stream = ledger_client
            .get_checkpoint(request)
            .await
            .expect("gRPC call to get_checkpoint");

        // Get metadata from the response
        let metadata = stream.metadata();
        verify_iota_headers(metadata, "get_checkpoint");

        // Verify epoch value
        let epoch = parse_u64_header(metadata, headers::X_IOTA_EPOCH);
        assert!(epoch >= 1, "epoch should be at least 1, got {epoch}");
    }

    // Test stream_checkpoints
    {
        let request = StreamCheckpointsRequest::default()
            .with_start_sequence_number(1)
            .with_end_sequence_number(2);

        let stream = ledger_client
            .stream_checkpoints(request)
            .await
            .expect("gRPC call to stream_checkpoints");

        // Get metadata from the response
        let metadata = stream.metadata();
        verify_iota_headers(metadata, "stream_checkpoints");

        // Verify epoch value
        let epoch = parse_u64_header(metadata, headers::X_IOTA_EPOCH);
        assert!(epoch >= 1, "epoch should be at least 1, got {epoch}");
    }
}
