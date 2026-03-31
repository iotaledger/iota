// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_types::v1::ledger_service::GetEpochRequest;
use iota_macros::sim_test;
use prost_types::FieldMask;

use crate::utils::setup_grpc_test;

#[sim_test]
async fn get_epoch() {
    let (_test_cluster, client) = setup_grpc_test(Some(1), None).await;

    let mut ledger_client = client.ledger_service_client();

    // Get current epoch (no epoch specified means current epoch)
    let latest_epoch_response = ledger_client
        .get_epoch(GetEpochRequest::default())
        .await
        .unwrap()
        .into_inner();

    let latest_epoch = latest_epoch_response.epoch.unwrap();

    // Get epoch 0
    let epoch_0_response = ledger_client
        .get_epoch(GetEpochRequest::default().with_epoch(0))
        .await
        .unwrap()
        .into_inner();

    let epoch_0 = epoch_0_response.epoch.unwrap();

    assert_eq!(latest_epoch.committee, epoch_0.committee);

    assert_eq!(epoch_0.epoch, Some(0));
    assert_eq!(epoch_0.first_checkpoint, Some(0));

    // Ensure that fetching the system state for the epoch works (using field mask)
    let epoch_with_bcs = ledger_client
        .get_epoch(GetEpochRequest::default().with_read_mask(FieldMask {
            paths: vec!["bcs_system_state".to_string()],
        }))
        .await
        .unwrap()
        .into_inner()
        .epoch
        .unwrap();
    assert!(epoch_with_bcs.bcs_system_state.is_some());
}
