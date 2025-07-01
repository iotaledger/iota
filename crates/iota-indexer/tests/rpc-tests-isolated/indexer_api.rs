// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[expect(dead_code)]
#[path = "../common/mod.rs"]
mod common;

#[cfg(feature = "pg_integration")]
#[cfg(test)]
mod indexer_api_tests_isolated {
    use iota_json_rpc_api::IndexerApiClient;
    use iota_json_rpc_types::EventFilter;

    use crate::common::{
        indexer_wait_for_checkpoint, indexer_wait_for_latest_checkpoint,
        start_test_cluster_with_read_write_indexer,
    };

    #[tokio::test]
    async fn query_validator_epoch_info_event() {
        let (cluster, store, client) = &start_test_cluster_with_read_write_indexer(
            Some("query_validator_epoch_info_event"),
            None,
            None,
        )
        .await;
        indexer_wait_for_checkpoint(store, 1).await;

        cluster.force_new_epoch().await;
        indexer_wait_for_latest_checkpoint(store, cluster).await;

        let result = client.query_events(EventFilter::MoveEventType("0x0000000000000000000000000000000000000000000000000000000000000003::validator_set::ValidatorEpochInfoEventV1".parse().unwrap()), None, None, None).await;
        assert!(result.is_ok());
        assert!(!result.unwrap().data.is_empty());

        let result = client
            .query_events(
                EventFilter::MoveEventType(
                    "0x3::validator_set::ValidatorEpochInfoEventV1"
                        .parse()
                        .unwrap(),
                ),
                None,
                None,
                None,
            )
            .await;
        assert!(result.is_ok());
        assert!(!result.unwrap().data.is_empty());

        let result = client
            .query_events(
                EventFilter::MoveEventType(
                    "0x0003::validator_set::ValidatorEpochInfoEventV1"
                        .parse()
                        .unwrap(),
                ),
                None,
                None,
                None,
            )
            .await;
        assert!(result.is_ok());
        assert!(!result.unwrap().data.is_empty());

        let result = client
            .query_events(
                EventFilter::MoveEventType(
                    "0x1::validator_set::ValidatorEpochInfoEventV1"
                        .parse()
                        .unwrap(),
                ),
                None,
                None,
                None,
            )
            .await;
        assert!(result.is_ok());
        assert!(result.unwrap().data.is_empty());
    }
}
