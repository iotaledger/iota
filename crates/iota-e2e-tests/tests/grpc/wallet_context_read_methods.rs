// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_macros::sim_test;
use test_cluster::TestClusterBuilder;

/// `get_object_ref`/`get_object_owner`/`get_reference_gas_price` return the
/// same values whether `WalletContext` uses the gRPC or the JSON-RPC
/// backend, against the same gRPC-enabled test cluster.
#[sim_test]
async fn get_object_ref_and_owner_agree_across_backends() {
    let test_cluster = TestClusterBuilder::new()
        .with_fullnode_enable_grpc_api(true)
        .with_num_validators(1)
        .build()
        .await;
    test_cluster.wait_for_checkpoint(1, None).await;

    let grpc_wallet = &test_cluster.wallet;
    let jsonrpc_wallet_path = grpc_wallet.config().path().to_path_buf();
    let jsonrpc_wallet = iota_sdk::wallet_context::WalletContext::new(&jsonrpc_wallet_path)
        .unwrap()
        .with_jsonrpc_backend();

    let (_, gas_ref) = grpc_wallet.get_one_gas_object().await.unwrap().unwrap();

    let grpc_ref = grpc_wallet.get_object_ref(gas_ref.object_id).await.unwrap();
    let jsonrpc_ref = jsonrpc_wallet
        .get_object_ref(gas_ref.object_id)
        .await
        .unwrap();
    assert_eq!(grpc_ref, jsonrpc_ref);

    let grpc_owner = grpc_wallet
        .get_object_owner(&gas_ref.object_id)
        .await
        .unwrap();
    let jsonrpc_owner = jsonrpc_wallet
        .get_object_owner(&gas_ref.object_id)
        .await
        .unwrap();
    assert_eq!(grpc_owner, jsonrpc_owner);

    let grpc_rgp = grpc_wallet.get_reference_gas_price().await.unwrap();
    let jsonrpc_rgp = jsonrpc_wallet.get_reference_gas_price().await.unwrap();
    assert_eq!(grpc_rgp, jsonrpc_rgp);
}

/// `gas_objects` returns `iota_sdk_types::Object`s whose gas balance and
/// object ref agree between the gRPC and JSON-RPC backends.
#[sim_test]
async fn gas_objects_agree_across_backends() {
    let test_cluster = TestClusterBuilder::new()
        .with_fullnode_enable_grpc_api(true)
        .with_num_validators(1)
        .build()
        .await;
    test_cluster.wait_for_checkpoint(1, None).await;

    let grpc_wallet = &test_cluster.wallet;
    let jsonrpc_wallet_path = grpc_wallet.config().path().to_path_buf();
    let jsonrpc_wallet = iota_sdk::wallet_context::WalletContext::new(&jsonrpc_wallet_path)
        .unwrap()
        .with_jsonrpc_backend();

    let address = grpc_wallet.active_address().unwrap();
    let mut grpc_coins = grpc_wallet.gas_objects(address).await.unwrap();
    let mut jsonrpc_coins = jsonrpc_wallet.gas_objects(address).await.unwrap();
    grpc_coins.sort_by_key(|(_, o)| o.id());
    jsonrpc_coins.sort_by_key(|(_, o)| o.id());

    assert_eq!(grpc_coins.len(), jsonrpc_coins.len());
    for ((grpc_value, grpc_object), (jsonrpc_value, jsonrpc_object)) in
        grpc_coins.iter().zip(jsonrpc_coins.iter())
    {
        assert_eq!(grpc_value, jsonrpc_value);
        assert_eq!(grpc_object.object_ref(), jsonrpc_object.object_ref());
    }
}
