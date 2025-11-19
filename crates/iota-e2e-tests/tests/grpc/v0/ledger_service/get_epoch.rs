// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_types::v0::ledger_service::{
    GetEpochRequest, ledger_service_client::LedgerServiceClient,
};
use iota_macros::sim_test;
use prost_types::FieldMask;
use test_cluster::TestClusterBuilder;

#[sim_test]
async fn get_epoch() {
    let test_cluster = TestClusterBuilder::new()
        .with_fullnode_enable_grpc_api(true)
        .build()
        .await;

    // Wait for at least one checkpoint to be created
    test_cluster.wait_for_checkpoint(1, None).await;

    let mut client = LedgerServiceClient::connect(test_cluster.grpc_url())
        .await
        .unwrap();

    // Get current epoch (no epoch specified means current epoch)
    let latest_epoch_response = client
        .get_epoch(GetEpochRequest {
            epoch: None,
            read_mask: None,
        })
        .await
        .unwrap()
        .into_inner();

    let latest_epoch = latest_epoch_response.epoch.unwrap();

    // Get epoch 0
    let epoch_0_response = client
        .get_epoch(GetEpochRequest {
            epoch: Some(0),
            read_mask: None,
        })
        .await
        .unwrap()
        .into_inner();

    let epoch_0 = epoch_0_response.epoch.unwrap();

    assert_eq!(latest_epoch.committee, epoch_0.committee);

    assert_eq!(epoch_0.epoch, Some(0));
    assert_eq!(epoch_0.first_checkpoint, Some(0));

    // Ensure that fetching the system state for the epoch works (using field mask)
    let epoch_with_bcs = client
        .get_epoch(GetEpochRequest {
            epoch: None,
            read_mask: Some(FieldMask {
                paths: vec!["bcs_system_state".to_string()],
            }),
        })
        .await
        .unwrap()
        .into_inner()
        .epoch
        .unwrap();
    assert!(epoch_with_bcs.bcs_system_state.is_some());
}
