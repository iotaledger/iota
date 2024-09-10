// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::time::Duration;

use iota_core::test_utils::wait_for_tx;
use iota_json_rpc_types::{
    IotaTransactionBlockEffects, IotaTransactionBlockEffectsAPI, TransactionFilter,
};
use iota_test_transaction_builder::{create_devnet_nft, publish_nfts_package};
use jsonrpsee::{
    core::client::{Subscription, SubscriptionClientT},
    rpc_params,
};
use test_cluster::TestClusterBuilder;
use tokio::time::timeout;

#[tokio::test]
async fn test_subscribe_transaction() -> Result<(), anyhow::Error> {
    let cluster = TestClusterBuilder::new().build().await;

    let address = &cluster.get_address_0();
    let wallet = cluster.wallet;

    let ws_client = cluster.fullnode_handle.ws_client().await;

    let package_id = publish_nfts_package(&wallet).await.0;

    let mut sub: Subscription<IotaTransactionBlockEffects> = ws_client
        .subscribe(
            "iotax_subscribeTransaction",
            rpc_params![TransactionFilter::FromAddress(*address)],
            "iotax_unsubscribeTransaction",
        )
        .await
        .unwrap();

    let (_, _, digest) = create_devnet_nft(&wallet, package_id).await;
    wait_for_tx(digest, cluster.fullnode_handle.iota_node.state()).await;

    // Wait for streaming
    let effects = match timeout(Duration::from_secs(5), sub.next()).await {
        Ok(Some(Ok(tx))) => tx,
        _ => panic!("Failed to get tx"),
    };

    assert_eq!(&digest, effects.transaction_digest());
    Ok(())
}
