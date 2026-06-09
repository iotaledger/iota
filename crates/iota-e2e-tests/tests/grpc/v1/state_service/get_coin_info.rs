// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_types::v1::{
    coin::{coin_treasury::SupplyState, regulated_coin_metadata::CoinRegulatedState},
    state_service::GetCoinInfoRequest,
};
use iota_macros::sim_test;

use crate::utils::{assert_field_presence, assert_tonic_error, setup_grpc_test};

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

    // GetCoinInfoRequest has no `read_mask`; the server populates every field
    // that exists for this coin. For the native IOTA coin (no on-chain
    // RegulatedCoinMetadata object), `regulated_metadata` is populated with
    // ONLY `coin_regulated_state = Unregulated` — the other 5 fields stay None.
    assert_field_presence(
        &response,
        &[
            "coin_type",
            "metadata.id",
            "metadata.decimals",
            "metadata.name",
            "metadata.symbol",
            "metadata.description",
            "metadata.icon_url",
            "treasury.id",
            "treasury.total_supply",
            "treasury.supply_state",
            "regulated_metadata.coin_regulated_state",
        ],
        &[],
        "get_coin_info iota",
    );

    // Spot-check stable values for the native coin.
    assert_eq!(response.coin_type.as_deref(), Some("0x2::iota::IOTA"));
    let metadata = response.metadata.as_ref().unwrap();
    assert_eq!(metadata.name.as_deref(), Some("IOTA"));
    assert_eq!(metadata.symbol.as_deref(), Some("IOTA"));
    assert_eq!(metadata.decimals, Some(9), "IOTA has 9 decimal places");
    assert_eq!(
        metadata.icon_url.as_deref(),
        Some("https://iota.org/logo.png"),
        "IOTA icon_url is set deterministically in genesis (see iota.move)"
    );
    let treasury = response.treasury.as_ref().unwrap();
    assert_eq!(
        treasury.supply_state,
        Some(SupplyState::Fixed as i32),
        "IOTA supply should be FIXED"
    );
    assert_eq!(
        response
            .regulated_metadata
            .as_ref()
            .unwrap()
            .coin_regulated_state,
        Some(CoinRegulatedState::Unregulated as i32),
        "native IOTA coin has no RegulatedCoinMetadata object → state is Unregulated"
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
