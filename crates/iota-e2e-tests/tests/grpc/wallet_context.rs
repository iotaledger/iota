// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_types::v1::ledger_service::GetServiceInfoRequest;
use iota_macros::sim_test;
use test_cluster::TestClusterBuilder;

/// `WalletContext::get_grpc_client()` builds a client from the active env's
/// `grpc` URL that can reach a live cluster's gRPC endpoint.
#[sim_test]
async fn wallet_context_get_grpc_client() {
    let mut test_cluster = TestClusterBuilder::new()
        .with_fullnode_enable_grpc_api(true)
        .with_num_validators(1)
        .build()
        .await;
    test_cluster.wait_for_checkpoint(1, None).await;

    // Point the wallet's active env at the cluster's gRPC endpoint.
    let grpc_url = test_cluster.grpc_url();
    let context = &mut test_cluster.wallet;
    let mut env = context.active_env().unwrap().clone();
    env.set_grpc(Some(grpc_url));
    context.config_mut().set_env(env);

    // The client is created from the wallet config, cached, and can make a
    // real call against the cluster.
    let client = context.get_grpc_client().await.unwrap();
    let info = client
        .ledger_service_client()
        .get_service_info(GetServiceInfoRequest::default())
        .await
        .unwrap()
        .into_inner();

    assert!(
        info.chain_id.is_some(),
        "get_service_info returned no chain id: {info:?}"
    );
}
