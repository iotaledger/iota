// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[expect(dead_code)]
#[path = "../common/mod.rs"]
mod common;

#[cfg(feature = "pg_integration")]
#[cfg(test)]
mod coin_api_tests_isolated {

    use iota_indexer::store::PgIndexerStore;
    use iota_json_rpc_api::CoinReadApiClient;
    use iota_json_rpc_types::IotaCoinMetadata;
    use iota_keys::keystore::AccountKeystore;
    use iota_types::{balance::Supply, base_types::IotaAddress, crypto::IotaKeyPair};
    use jsonrpsee::http_client::HttpClient;
    use test_cluster::TestCluster;

    use crate::common::{
        indexer_wait_for_transaction,
        rpc_tests::{
            create_migrated_coin_manager_coins, get_total_supply_fullnode_indexer,
            publish_test_move_package,
        },
        start_test_cluster_with_read_write_indexer,
    };

    #[tokio::test]
    async fn indexer_get_coin_metadata_with_migrated_coin_manager_coins() {
        let (cluster, store, client) = &start_test_cluster_with_read_write_indexer(
            Some("indexer_get_coin_metadata_with_migrated_coin_manager_coins"),
            None,
            None,
        )
        .await;

        let address = cluster.wallet.active_address().unwrap();
        let address_kp = cluster
            .wallet
            .config()
            .keystore()
            .get_key(&address)
            .unwrap();
        let (coin_name, immutable_metadata_coin_name) = create_migrated_coin_manager_coins(
            cluster,
            client,
            store,
            address,
            address_kp.as_keypair().unwrap(),
        )
        .await
        .unwrap();

        let (_, result_indexer) =
            get_coin_metadata_fullnode_indexer(cluster, client, coin_name.to_string()).await;

        assert!(result_indexer.is_some());
        let result_indexer = result_indexer.unwrap();
        assert_eq!(result_indexer.decimals, 2);
        assert_eq!(result_indexer.name, "Trusted Coin");
        assert_eq!(result_indexer.symbol, "TRUSTED");
        assert_eq!(result_indexer.description, "Trusted Coin for test");
        assert_eq!(result_indexer.icon_url, None);
        assert!(result_indexer.id.is_some());

        let (_, result_indexer) = get_coin_metadata_fullnode_indexer(
            cluster,
            client,
            immutable_metadata_coin_name.to_string(),
        )
        .await;

        assert!(result_indexer.is_some());
        let result_indexer = result_indexer.unwrap();
        assert_eq!(result_indexer.decimals, 2);
        assert_eq!(result_indexer.name, "Immutable Metadata Trusted Coin");
        assert_eq!(result_indexer.symbol, "IMM_META_TRUSTED");
        assert_eq!(
            result_indexer.description,
            "Immutable Metadata Trusted Coin for test"
        );
        assert_eq!(result_indexer.icon_url, None);
        assert!(result_indexer.id.is_none()); // Immutable data is stored in struct that doesn't have ID
    }

    #[tokio::test]
    async fn get_coin_metadata_with_native_coin_manager_coins() {
        let (cluster, store, client) = &start_test_cluster_with_read_write_indexer(
            Some("get_coin_metadata_with_native_coin_manager_coins"),
            None,
            None,
        )
        .await;

        let address = cluster.wallet.active_address().unwrap();
        let address_kp = cluster
            .wallet
            .config()
            .keystore()
            .get_key(&address)
            .unwrap();
        let (coin_name, immutable_metadata_coin_name) = create_native_coin_manager_coins(
            cluster,
            client,
            store,
            address,
            address_kp.as_keypair().unwrap(),
        )
        .await
        .unwrap();

        let (result_fullnode, result_indexer) =
            get_coin_metadata_fullnode_indexer(cluster, client, coin_name.to_string()).await;

        assert!(result_indexer.is_some());
        assert_eq!(result_fullnode, result_indexer);
        assert!(result_indexer.unwrap().id.is_some());

        let (result_fullnode, result_indexer) = get_coin_metadata_fullnode_indexer(
            cluster,
            client,
            immutable_metadata_coin_name.to_string(),
        )
        .await;

        assert!(result_indexer.is_some());
        assert_eq!(result_fullnode, result_indexer);
        assert!(result_indexer.unwrap().id.is_none()); // Immutable data is stored in struct that doesn't have ID
    }

    async fn get_coin_metadata_fullnode_indexer(
        cluster: &TestCluster,
        client: &HttpClient,
        coin_type: String,
    ) -> (Option<IotaCoinMetadata>, Option<IotaCoinMetadata>) {
        let result_fullnode = cluster
            .rpc_client()
            .get_coin_metadata(coin_type.clone())
            .await
            .unwrap();
        let result_indexer = client.get_coin_metadata(coin_type).await.unwrap();
        (result_fullnode, result_indexer)
    }

    #[tokio::test]
    async fn indexer_get_total_supply_with_migrated_coin_manager_coins() {
        let (cluster, store, client) = &start_test_cluster_with_read_write_indexer(
            Some("indexer_get_total_supply_with_migrated_coin_manager_coins"),
            None,
            None,
        )
        .await;

        let address = cluster.wallet.active_address().unwrap();
        let address_kp = cluster
            .wallet
            .config()
            .keystore()
            .get_key(&address)
            .unwrap();
        let (coin_name, immutable_metadata_coin_name) = create_migrated_coin_manager_coins(
            cluster,
            client,
            store,
            address,
            address_kp.as_keypair().unwrap(),
        )
        .await
        .unwrap();

        let (_, result_indexer) =
            get_total_supply_fullnode_indexer(cluster, client, coin_name.to_string()).await;
        assert_eq!(result_indexer, Some(Supply { value: 100_000 }));

        let (_, result_indexer) = get_total_supply_fullnode_indexer(
            cluster,
            client,
            immutable_metadata_coin_name.to_string(),
        )
        .await;
        assert_eq!(result_indexer, Some(Supply { value: 0 }));
    }

    #[tokio::test]
    async fn get_total_supply_with_native_coin_manager_coins() {
        let (cluster, store, client) = &start_test_cluster_with_read_write_indexer(
            Some("get_total_supply_with_native_coin_manager_coins"),
            None,
            None,
        )
        .await;

        let address = cluster.wallet.active_address().unwrap();
        let address_kp = cluster
            .wallet
            .config()
            .keystore()
            .get_key(&address)
            .unwrap();
        let (coin_name, immutable_metadata_coin_name) = create_native_coin_manager_coins(
            cluster,
            client,
            store,
            address,
            address_kp.as_keypair().unwrap(),
        )
        .await
        .unwrap();

        let (result_fullnode, result_indexer) =
            get_total_supply_fullnode_indexer(cluster, client, coin_name.to_string()).await;
        assert_eq!(result_indexer, Some(Supply { value: 0 }));
        assert_eq!(result_fullnode, result_indexer);

        let (result_fullnode, result_indexer) = get_total_supply_fullnode_indexer(
            cluster,
            client,
            immutable_metadata_coin_name.to_string(),
        )
        .await;
        assert_eq!(result_indexer, Some(Supply { value: 0 }));
        assert_eq!(result_fullnode, result_indexer);
    }

    async fn create_native_coin_manager_coins(
        cluster: &TestCluster,
        indexer_client: &HttpClient,
        pg_store: &PgIndexerStore,
        address: IotaAddress,
        account_keypair: &IotaKeyPair,
    ) -> Result<(String, String), anyhow::Error> {
        let http_client = cluster.rpc_client();

        let (package_id, tx_response) =
            publish_test_move_package(http_client, address, account_keypair, "coin_manager_coins")
                .await?;
        indexer_wait_for_transaction(tx_response.digest, pg_store, indexer_client).await;

        let coin_name = format!("{package_id}::normal_coin::NORMAL_COIN");
        let immutable_metadata_coin_name =
            format!("{package_id}::immutable_metadata_coin::IMMUTABLE_METADATA_COIN");
        Ok((coin_name, immutable_metadata_coin_name))
    }
}
