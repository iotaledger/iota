// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_types::v1::state_service::GetCoinInfoRequest;
use iota_macros::sim_test;

use crate::utils::{assert_tonic_error, setup_grpc_test};

#[sim_test]
async fn get_coin_info_iota() {
    let (_test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let mut state_client = client.state_service_client();

    let request = GetCoinInfoRequest::default().with_coin_type("0x2::iota::IOTA".to_string());

    let response = state_client
        .get_coin_info(request)
        .await
        .unwrap()
        .into_inner();

    // IOTA coin should have a coin_type set
    assert_eq!(
        response.coin_type.as_deref(),
        Some("0x2::iota::IOTA"),
        "coin_type should be 0x2::iota::IOTA"
    );

    // IOTA coin should have metadata with correct content
    let metadata = response
        .metadata
        .as_ref()
        .expect("IOTA coin should have metadata");
    assert!(metadata.id.is_some(), "metadata should have an id");
    assert_eq!(metadata.name.as_deref(), Some("IOTA"));
    assert_eq!(metadata.symbol.as_deref(), Some("IOTA"));
    assert!(
        metadata.description.is_some(),
        "metadata should have a description"
    );
    assert_eq!(metadata.decimals, Some(9), "IOTA has 9 decimal places");

    // IOTA coin should have treasury with correct content
    let treasury = response
        .treasury
        .as_ref()
        .expect("IOTA coin should have treasury info");
    assert!(treasury.id.is_some(), "treasury should have an id");
    assert!(
        treasury.total_supply.is_some(),
        "treasury should have total_supply"
    );
    // IOTA native gas coin has fixed supply
    assert_eq!(
        treasury.supply_state,
        Some(1),
        "IOTA supply should be FIXED (1)"
    );
}

#[sim_test]
async fn get_coin_info_missing_coin_type() {
    let (_test_cluster, client) = setup_grpc_test(None, None).await;
    let mut state_client = client.state_service_client();

    // Missing coin_type should return InvalidArgument
    let result = state_client
        .get_coin_info(GetCoinInfoRequest::default())
        .await;

    assert_tonic_error(result, tonic::Code::InvalidArgument, "missing coin_type");
}

#[sim_test]
async fn get_coin_info_invalid_coin_type() {
    let (_test_cluster, client) = setup_grpc_test(None, None).await;
    let mut state_client = client.state_service_client();

    // Invalid struct tag should return InvalidArgument
    let request = GetCoinInfoRequest::default().with_coin_type("not_a_valid_type".to_string());

    let result = state_client.get_coin_info(request).await;

    assert_tonic_error(result, tonic::Code::InvalidArgument, "invalid coin_type");
}

#[sim_test]
async fn get_coin_info_nonexistent_coin() {
    let (_test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let mut state_client = client.state_service_client();

    // Valid struct tag but coin type doesn't exist on chain — server returns
    // NotFound
    let request = GetCoinInfoRequest::default().with_coin_type("0x1234::fake::FAKE".to_string());

    let result = state_client.get_coin_info(request).await;

    assert_tonic_error(result, tonic::Code::NotFound, "nonexistent coin type");
}
