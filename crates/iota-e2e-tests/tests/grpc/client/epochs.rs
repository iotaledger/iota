// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_macros::sim_test;

use super::common::setup_grpc_test;

#[sim_test]
async fn get_reference_gas_price() {
    let (_test_cluster, client) = setup_grpc_test(1).await;

    let gas_price = client
        .get_reference_gas_price()
        .await
        .expect("Failed to get reference gas price");

    assert!(gas_price > 0, "Reference gas price should be positive");
    assert!(
        gas_price <= 10_000_000,
        "Reference gas price {gas_price} seems unreasonably high"
    );
}
