// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[expect(dead_code)]
#[path = "../common/mod.rs"]
mod common;

#[cfg(feature = "pg_integration")]
#[cfg(test)]
mod read_api_tests_isolated {
    use iota_json_rpc_api::ReadApiClient;
    use iota_json_rpc_types::CheckpointId;
    use iota_types::digests::ChainIdentifier;

    use crate::common::{
        indexer_wait_for_checkpoint, indexer_wait_for_checkpoint_pruned,
        start_test_cluster_with_read_write_indexer,
    };

    #[tokio::test]
    async fn get_chain_identifier_with_pruning_enabled() {
        let (cluster, store, client) = &start_test_cluster_with_read_write_indexer(
            Some("test_get_chain_identifier_with_pruning_enabled"),
            None,
            Some(1),
        )
        .await;

        indexer_wait_for_checkpoint(store, 1).await;

        let chain_identifier = ChainIdentifier::from(
            client
                .get_checkpoint(CheckpointId::SequenceNumber(0))
                .await
                .unwrap()
                .digest,
        );

        let indexer_chain_identifier = client.get_chain_identifier().await.unwrap();

        assert_eq!(
            chain_identifier.to_string(),
            indexer_chain_identifier.to_string()
        );

        cluster.force_new_epoch().await;

        // Prune the genesis checkpoint
        indexer_wait_for_checkpoint_pruned(store, 0).await;

        let indexer_chain_identifier = client.get_chain_identifier().await.unwrap();

        assert_eq!(
            chain_identifier.to_string(),
            indexer_chain_identifier.to_string()
        );

        assert!(
            client
                .get_checkpoint(CheckpointId::SequenceNumber(0))
                .await
                .is_err()
        )
    }
}
