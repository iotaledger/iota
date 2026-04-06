// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_types::{
    field::FieldMaskUtil,
    read_masks::GET_SERVICE_INFO_READ_MASK,
    v1::ledger_service::{
        GetServiceInfoRequest, GetServiceInfoResponse, ledger_service_client::LedgerServiceClient,
    },
};
use iota_macros::sim_test;
use prost_types::FieldMask;

use crate::utils::{assert_field_presence, comma_separated_field_mask_to_paths, setup_grpc_test};

async fn assert_service_info_request(
    ledger_client: &mut LedgerServiceClient<iota_grpc_client::InterceptedChannel>,
    read_mask: Option<FieldMask>,
    expected_fields: &[&str],
    scenario: &str,
) -> GetServiceInfoResponse {
    let response = ledger_client
        .get_service_info({
            let mut req = GetServiceInfoRequest::default();
            if let Some(mask) = read_mask {
                req = req.with_read_mask(mask);
            }
            req
        })
        .await
        .unwrap()
        .into_inner();

    assert_field_presence(&response, expected_fields, &[], scenario);
    response
}

#[sim_test]
async fn get_service_info_readmask_scenarios() {
    let (_test_cluster, client) = setup_grpc_test(Some(1), None).await;

    let mut ledger_client = client.ledger_service_client();

    // Test 1: Default readmask (None) should return only default fields:
    // chain_id, epoch, executed_checkpoint_height
    assert_service_info_request(
        &mut ledger_client,
        None,
        &comma_separated_field_mask_to_paths(GET_SERVICE_INFO_READ_MASK),
        "default readmask",
    )
    .await;

    // Test 2: Empty readmask should return no fields
    assert_service_info_request(
        &mut ledger_client,
        Some(FieldMask::from_paths(&[] as &[&str])),
        &[],
        "empty readmask",
    )
    .await;

    // Test 3: Full readmask should return all fields
    assert_service_info_request(
        &mut ledger_client,
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
        &mut ledger_client,
        Some(FieldMask::from_paths(["chain_id", "server"])),
        &["chain_id", "server"],
        "partial readmask",
    )
    .await;
}
