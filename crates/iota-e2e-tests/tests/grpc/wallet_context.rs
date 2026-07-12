// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_macros::sim_test;
use test_cluster::TestClusterBuilder;

/// `WalletContext::get_grpc_client()` returns a working client for a
/// gRPC-enabled cluster.
#[sim_test]
async fn wallet_context_get_grpc_client() {
    let test_cluster = TestClusterBuilder::new()
        .with_num_validators(1)
        .build()
        .await;
    test_cluster.wait_for_checkpoint(1, None).await;

    let info = test_cluster
        .wallet
        .get_grpc_client()
        .await
        .unwrap()
        .get_service_info(None)
        .await
        .unwrap();

    assert!(
        info.body().chain_id.is_some(),
        "get_service_info returned no chain id: {:?}",
        info.body()
    );
}

/// `TestClusterBuilder::new()` enables the fullnode's gRPC API by default,
/// so a plain `test_cluster.wallet` resolves to the gRPC backend without any
/// opt-in.
#[sim_test]
async fn test_cluster_wallet_defaults_to_grpc_backend() {
    let test_cluster = TestClusterBuilder::new()
        .with_num_validators(1)
        .build()
        .await;
    test_cluster.wait_for_checkpoint(1, None).await;

    assert!(
        test_cluster.wallet.active_env().unwrap().grpc().is_some(),
        "expected TestClusterBuilder::new() to configure a grpc URL by default"
    );
}

/// The gRPC and JSON-RPC backends return equivalent SDK-native values for
/// the same on-chain state, against the same (now gRPC-by-default)
/// test-cluster node.
#[sim_test]
async fn grpc_and_jsonrpc_backends_agree_end_to_end() {
    let test_cluster = TestClusterBuilder::new()
        .with_num_validators(1)
        .build()
        .await;
    test_cluster.wait_for_checkpoint(1, None).await;

    let grpc_wallet = &test_cluster.wallet;
    let jsonrpc_wallet_path = grpc_wallet.config().path().to_path_buf();
    let jsonrpc_wallet = iota_sdk::wallet_context::WalletContext::new(&jsonrpc_wallet_path)
        .unwrap()
        .with_jsonrpc_backend();

    let (sender, gas) = grpc_wallet.get_one_gas_object().await.unwrap().unwrap();

    assert_eq!(
        grpc_wallet.get_object_ref(gas.object_id).await.unwrap(),
        jsonrpc_wallet.get_object_ref(gas.object_id).await.unwrap(),
    );
    assert_eq!(
        grpc_wallet.get_object_owner(&gas.object_id).await.unwrap(),
        jsonrpc_wallet
            .get_object_owner(&gas.object_id)
            .await
            .unwrap(),
    );
    assert_eq!(
        grpc_wallet.get_reference_gas_price().await.unwrap(),
        jsonrpc_wallet.get_reference_gas_price().await.unwrap(),
    );

    let rgp = grpc_wallet.get_reference_gas_price().await.unwrap();
    let tx = grpc_wallet.sign_transaction(
        &iota_test_transaction_builder::TestTransactionBuilder::new(sender, gas, rgp)
            .transfer_iota(None, sender)
            .build(),
    );
    let grpc_result = grpc_wallet.execute_transaction_must_succeed(tx).await;
    assert!(
        grpc_result
            .effects()
            .unwrap()
            .effects()
            .unwrap()
            .as_v1()
            .status
            .is_success()
    );

    let (_, gas2) = grpc_wallet.get_one_gas_object().await.unwrap().unwrap();
    let tx2 = jsonrpc_wallet.sign_transaction(
        &iota_test_transaction_builder::TestTransactionBuilder::new(sender, gas2, rgp)
            .transfer_iota(None, sender)
            .build(),
    );
    let jsonrpc_result = jsonrpc_wallet.execute_transaction_must_succeed(tx2).await;
    assert!(
        jsonrpc_result
            .effects()
            .unwrap()
            .effects()
            .unwrap()
            .as_v1()
            .status
            .is_success()
    );
}
