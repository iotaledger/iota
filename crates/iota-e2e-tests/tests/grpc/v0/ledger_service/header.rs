// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_types::{
    headers,
    v0::ledger_service::{
        CheckpointDataStreamRequest, GetCheckpointDataRequest, GetEpochRequest, GetObjectsRequest,
        GetServiceInfoRequest, GetTransactionsRequest, get_checkpoint_data_request::CheckpointId,
    },
};
use iota_macros::sim_test;

use crate::{
    utils::setup_grpc_test_with_builder,
    v0::header::{parse_u64_header, verify_iota_headers},
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

        let request = GetServiceInfoRequest { read_mask: None };

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

        let request = GetEpochRequest {
            epoch: Some(1),
            read_mask: None,
        };
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
        let request = GetObjectsRequest {
            requests: None,
            read_mask: None,
            max_message_size_bytes: None,
        };

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
        let request = GetTransactionsRequest {
            requests: None,
            read_mask: None,
            max_message_size_bytes: None,
        };

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

    // Test get_checkpoint_data
    {
        let request = GetCheckpointDataRequest {
            checkpoint_id: Some(CheckpointId::Latest(true)),
            read_mask: None,
            transactions_filter: None,
            events_filter: None,
            max_message_size_bytes: None,
        };

        let stream = ledger_client
            .get_checkpoint_data(request)
            .await
            .expect("gRPC call to get_checkpoint_data");

        // Get metadata from the response
        let metadata = stream.metadata();
        verify_iota_headers(metadata, "get_checkpoint_data");

        // Verify epoch value
        let epoch = parse_u64_header(metadata, headers::X_IOTA_EPOCH);
        assert!(epoch >= 1, "epoch should be at least 1, got {epoch}");
    }

    // Test stream_checkpoint_data
    {
        let request = CheckpointDataStreamRequest {
            start_sequence_number: Some(1),
            end_sequence_number: Some(2),
            read_mask: None,
            transactions_filter: None,
            events_filter: None,
            max_message_size_bytes: None,
        };

        let stream = ledger_client
            .stream_checkpoint_data(request)
            .await
            .expect("gRPC call to stream_checkpoint_data");

        // Get metadata from the response
        let metadata = stream.metadata();
        verify_iota_headers(metadata, "stream_checkpoint_data");

        // Verify epoch value
        let epoch = parse_u64_header(metadata, headers::X_IOTA_EPOCH);
        assert!(epoch >= 1, "epoch should be at least 1, got {epoch}");
    }
}
