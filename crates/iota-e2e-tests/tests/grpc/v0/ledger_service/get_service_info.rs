// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_types::v0::ledger_service::{
    GetServiceInfoRequest, GetServiceInfoResponse, ledger_service_client::LedgerServiceClient,
};
use iota_macros::sim_test;
use prost_types::FieldMask;
use test_cluster::TestClusterBuilder;

#[sim_test]
async fn get_service_info_with_default_readmask() {
    let test_cluster = TestClusterBuilder::new()
        .with_fullnode_enable_grpc_api(true)
        .build()
        .await;

    // Wait for at least one checkpoint to be created
    test_cluster.wait_for_checkpoint(1, None).await;

    let mut grpc_client = LedgerServiceClient::connect(test_cluster.grpc_url())
        .await
        .unwrap();

    // Request with None readmask should return only default fields:
    // chain_id, epoch, executed_checkpoint_height
    let GetServiceInfoResponse {
        chain_id,
        chain,
        epoch,
        executed_checkpoint_height,
        executed_checkpoint_timestamp,
        lowest_available_checkpoint,
        lowest_available_checkpoint_objects,
        server,
        ..
    } = grpc_client
        .get_service_info(GetServiceInfoRequest { read_mask: None })
        .await
        .unwrap()
        .into_inner();

    // Default fields should be present
    assert!(
        chain_id.is_some(),
        "chain_id should be present in default mask"
    );
    assert!(epoch.is_some(), "epoch should be present in default mask");
    assert!(
        executed_checkpoint_height.is_some(),
        "executed_checkpoint_height should be present in default mask"
    );

    // Non-default fields should be None
    assert!(
        chain.is_none(),
        "chain should not be present in default mask"
    );
    assert!(
        executed_checkpoint_timestamp.is_none(),
        "executed_checkpoint_timestamp should not be present in default mask"
    );
    assert!(
        lowest_available_checkpoint.is_none(),
        "lowest_available_checkpoint should not be present in default mask"
    );
    assert!(
        lowest_available_checkpoint_objects.is_none(),
        "lowest_available_checkpoint_objects should not be present in default mask"
    );
    assert!(
        server.is_none(),
        "server should not be present in default mask"
    );
}

#[sim_test]
async fn get_service_info_without_readmask() {
    let test_cluster = TestClusterBuilder::new()
        .with_fullnode_enable_grpc_api(true)
        .build()
        .await;

    // Wait for at least one checkpoint to be created
    test_cluster.wait_for_checkpoint(1, None).await;

    let mut grpc_client = LedgerServiceClient::connect(test_cluster.grpc_url())
        .await
        .unwrap();

    // Request with empty readmask (empty paths) should return no fields
    let GetServiceInfoResponse {
        chain_id,
        chain,
        epoch,
        executed_checkpoint_height,
        executed_checkpoint_timestamp,
        lowest_available_checkpoint,
        lowest_available_checkpoint_objects,
        server,
        ..
    } = grpc_client
        .get_service_info(GetServiceInfoRequest {
            read_mask: Some(FieldMask { paths: vec![] }),
        })
        .await
        .unwrap()
        .into_inner();

    // All fields should be None with empty readmask
    assert!(
        chain_id.is_none(),
        "chain_id should not be present with empty mask"
    );
    assert!(
        chain.is_none(),
        "chain should not be present with empty mask"
    );
    assert!(
        epoch.is_none(),
        "epoch should not be present with empty mask"
    );
    assert!(
        executed_checkpoint_height.is_none(),
        "executed_checkpoint_height should not be present with empty mask"
    );
    assert!(
        executed_checkpoint_timestamp.is_none(),
        "executed_checkpoint_timestamp should not be present with empty mask"
    );
    assert!(
        lowest_available_checkpoint.is_none(),
        "lowest_available_checkpoint should not be present with empty mask"
    );
    assert!(
        lowest_available_checkpoint_objects.is_none(),
        "lowest_available_checkpoint_objects should not be present with empty mask"
    );
    assert!(
        server.is_none(),
        "server should not be present with empty mask"
    );
}

#[sim_test]
async fn get_service_info_with_full_readmask() {
    let test_cluster = TestClusterBuilder::new()
        .with_fullnode_enable_grpc_api(true)
        .build()
        .await;

    // Wait for at least one checkpoint to be created
    test_cluster.wait_for_checkpoint(1, None).await;

    let mut grpc_client = LedgerServiceClient::connect(test_cluster.grpc_url())
        .await
        .unwrap();

    // Request with full readmask should return all fields
    let GetServiceInfoResponse {
        chain_id,
        chain,
        epoch,
        executed_checkpoint_height,
        executed_checkpoint_timestamp,
        lowest_available_checkpoint,
        lowest_available_checkpoint_objects,
        server,
        ..
    } = grpc_client
        .get_service_info(GetServiceInfoRequest {
            read_mask: Some(FieldMask {
                paths: vec![
                    "chain_id".to_string(),
                    "chain".to_string(),
                    "epoch".to_string(),
                    "executed_checkpoint_height".to_string(),
                    "executed_checkpoint_timestamp".to_string(),
                    "lowest_available_checkpoint".to_string(),
                    "lowest_available_checkpoint_objects".to_string(),
                    "server".to_string(),
                ],
            }),
        })
        .await
        .unwrap()
        .into_inner();

    // All fields should be present with full readmask
    assert!(
        chain_id.is_some(),
        "chain_id should be present with full mask"
    );
    assert!(chain.is_some(), "chain should be present with full mask");
    assert!(epoch.is_some(), "epoch should be present with full mask");
    assert!(
        executed_checkpoint_height.is_some(),
        "executed_checkpoint_height should be present with full mask"
    );
    assert!(
        executed_checkpoint_timestamp.is_some(),
        "executed_checkpoint_timestamp should be present with full mask"
    );
    assert!(
        lowest_available_checkpoint.is_some(),
        "lowest_available_checkpoint should be present with full mask"
    );
    assert!(
        lowest_available_checkpoint_objects.is_some(),
        "lowest_available_checkpoint_objects should be present with full mask"
    );
    assert!(server.is_some(), "server should be present with full mask");
}
