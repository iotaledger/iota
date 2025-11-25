// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_types::{
    field::FieldMaskUtil,
    v0::ledger_service::{
        GetServiceInfoRequest, GetServiceInfoResponse, ledger_service_client::LedgerServiceClient,
    },
};
use iota_macros::sim_test;
use prost_types::FieldMask;
use test_cluster::TestClusterBuilder;

use crate::{impl_field_presence_checker, utils::assert_field_presence};

// Generate the FieldPresenceChecker implementation for GetServiceInfoResponse
impl_field_presence_checker!(GetServiceInfoResponse {
    chain_id,
    chain,
    epoch,
    executed_checkpoint_height,
    executed_checkpoint_timestamp,
    lowest_available_checkpoint,
    lowest_available_checkpoint_objects,
    server,
});

async fn assert_service_info_request(
    client: &mut LedgerServiceClient<tonic::transport::Channel>,
    read_mask: Option<FieldMask>,
    expected_fields: &[&str],
    scenario: &str,
) -> GetServiceInfoResponse {
    let response = client
        .get_service_info(GetServiceInfoRequest { read_mask })
        .await
        .unwrap()
        .into_inner();

    assert_field_presence(&response, expected_fields, scenario);
    response
}

#[sim_test]
async fn get_service_info_readmask_scenarios() {
    let test_cluster = TestClusterBuilder::new()
        .with_fullnode_enable_grpc_api(true)
        .build()
        .await;

    // Wait for at least one checkpoint to be created
    test_cluster.wait_for_checkpoint(1, None).await;

    let mut grpc_client = LedgerServiceClient::connect(test_cluster.grpc_url())
        .await
        .unwrap();

    // Test 1: Default readmask (None) should return only default fields:
    // chain_id, epoch, executed_checkpoint_height
    assert_service_info_request(
        &mut grpc_client,
        None,
        &["chain_id", "epoch", "executed_checkpoint_height"],
        "default readmask",
    )
    .await;

    // Test 2: Empty readmask should return no fields
    assert_service_info_request(
        &mut grpc_client,
        Some(FieldMask::from_paths(&[] as &[&str])),
        &[],
        "empty readmask",
    )
    .await;

    // Test 3: Full readmask should return all fields
    assert_service_info_request(
        &mut grpc_client,
        Some(FieldMask::from_paths([
            "chain_id",
            "chain",
            "epoch",
            "executed_checkpoint_height",
            "executed_checkpoint_timestamp",
            "lowest_available_checkpoint",
            "lowest_available_checkpoint_objects",
            "server",
        ])),
        &[
            "chain_id",
            "chain",
            "epoch",
            "executed_checkpoint_height",
            "executed_checkpoint_timestamp",
            "lowest_available_checkpoint",
            "lowest_available_checkpoint_objects",
            "server",
        ],
        "full readmask",
    )
    .await;

    // Test 4: Partial readmask should return only requested fields
    assert_service_info_request(
        &mut grpc_client,
        Some(FieldMask::from_paths(["chain_id", "server"])),
        &["chain_id", "server"],
        "partial readmask",
    )
    .await;
}
