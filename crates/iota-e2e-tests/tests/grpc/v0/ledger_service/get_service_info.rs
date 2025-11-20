// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_types::v0::ledger_service::{
    GetServiceInfoRequest, GetServiceInfoResponse, ledger_service_client::LedgerServiceClient,
};
use iota_macros::sim_test;
use test_cluster::TestClusterBuilder;

#[sim_test]
async fn get_service_info() {
    let test_cluster = TestClusterBuilder::new()
        .with_fullnode_enable_grpc_api(true)
        .build()
        .await;

    // Wait for at least one checkpoint to be created
    test_cluster.wait_for_checkpoint(1, None).await;

    let mut grpc_client = LedgerServiceClient::connect(test_cluster.grpc_url())
        .await
        .unwrap();

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
        .get_service_info(GetServiceInfoRequest::default())
        .await
        .unwrap()
        .into_inner();

    assert!(chain_id.is_some());
    assert!(chain.is_some());
    assert!(epoch.is_some());
    assert!(executed_checkpoint_height.is_some());
    assert!(executed_checkpoint_timestamp.is_some());
    assert!(lowest_available_checkpoint.is_some());
    assert!(lowest_available_checkpoint_objects.is_some());
    assert!(server.is_some());
}
