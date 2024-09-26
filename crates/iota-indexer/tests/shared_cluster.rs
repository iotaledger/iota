// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[cfg(feature = "pg_integration")]
mod common;

#[cfg(feature = "pg_integration")]
mod integration_tests {
    use std::str::FromStr;

    use iota_json_rpc_api::{IndexerApiClient, ReadApiClient};
    use iota_json_rpc_types::{
        CheckpointId, IotaGetPastObjectRequest, IotaObjectDataOptions, IotaObjectResponse,
        IotaObjectResponseQuery, IotaTransactionBlockResponse, IotaTransactionBlockResponseOptions,
    };
    use iota_types::{
        base_types::{ObjectID, SequenceNumber},
        digests::TransactionDigest,
        error::IotaObjectResponseError,
    };

    use crate::common::pg_integration::{
        get_global_test_cluster_with_read_write_indexer, indexer_wait_for_checkpoint,
        rpc_call_error_msg_matches,
    };

    fn is_ascending(vec: &[u64]) -> bool {
        vec.windows(2).all(|window| window[0] <= window[1])
    }
    fn is_descending(vec: &[u64]) -> bool {
        vec.windows(2).all(|window| window[0] >= window[1])
    }

    /// Checks if
    /// [`iota_json_rpc_types::IotaTransactionBlockResponse`] match to the
    /// provided
    /// [`iota_json_rpc_types::IotaTransactionBlockResponseOptions`] filters
    fn match_transaction_block_resp_options(
        expected_options: &IotaTransactionBlockResponseOptions,
        responses: &[IotaTransactionBlockResponse],
    ) -> bool {
        responses
            .iter()
            .map(|iota_tx_block_resp| IotaTransactionBlockResponseOptions {
                show_input: iota_tx_block_resp.transaction.is_some(),
                show_raw_input: !iota_tx_block_resp.raw_transaction.is_empty(),
                show_effects: iota_tx_block_resp.effects.is_some(),
                show_events: iota_tx_block_resp.events.is_some(),
                show_object_changes: iota_tx_block_resp.object_changes.is_some(),
                show_balance_changes: iota_tx_block_resp.balance_changes.is_some(),
                show_raw_effects: !iota_tx_block_resp.raw_effects.is_empty(),
            })
            .all(|actual_options| actual_options.eq(expected_options))
    }

    fn get_object_with_options(options: IotaObjectDataOptions) {
        let (runtime, (cluster, pg_store, indexer_client)) =
            get_global_test_cluster_with_read_write_indexer();

        runtime.block_on(async move {
            indexer_wait_for_checkpoint(pg_store, 1).await;
            let address = cluster.get_address_0();

            let fullnode_objects = cluster
                .rpc_client()
                .get_owned_objects(
                    address,
                    Some(IotaObjectResponseQuery::new_with_options(options.clone())),
                    None,
                    None,
                )
                .await
                .unwrap();

            for obj in fullnode_objects.data {
                let indexer_obj = indexer_client
                    .get_object(obj.object_id().unwrap(), Some(options.clone()))
                    .await
                    .unwrap();

                assert_eq!(obj, indexer_obj);
            }
        });
    }

    fn multi_get_objects_with_options(options: IotaObjectDataOptions) {
        let (runtime, (cluster, pg_store, indexer_client)) =
            get_global_test_cluster_with_read_write_indexer();

        runtime.block_on(async move {
            indexer_wait_for_checkpoint(pg_store, 1).await;
            let address = cluster.get_address_0();

            let fullnode_objects = cluster
                .rpc_client()
                .get_owned_objects(
                    address,
                    Some(IotaObjectResponseQuery::new_with_options(options.clone())),
                    None,
                    None,
                )
                .await
                .unwrap();

            let object_ids = fullnode_objects
                .data
                .iter()
                .map(|iota_object| iota_object.object_id().unwrap())
                .collect::<Vec<ObjectID>>();

            let indexer_objects = indexer_client
                .multi_get_objects(object_ids, Some(options))
                .await
                .unwrap();

            assert_eq!(fullnode_objects.data, indexer_objects);
        });
    }

    fn get_transaction_block_with_options(options: IotaTransactionBlockResponseOptions) {
        let (runtime, (cluster, pg_store, indexer_client)) =
            get_global_test_cluster_with_read_write_indexer();

        runtime.block_on(async move {
            indexer_wait_for_checkpoint(pg_store, 1).await;

            let fullnode_checkpoint = cluster
                .rpc_client()
                .get_checkpoint(CheckpointId::SequenceNumber(0))
                .await
                .unwrap();

            let tx_digest = *fullnode_checkpoint.transactions.first().unwrap();

            let fullnode_tx = cluster
                .rpc_client()
                .get_transaction_block(tx_digest, Some(options.clone()))
                .await
                .unwrap();

            let tx = indexer_client
                .get_transaction_block(tx_digest, Some(options.clone()))
                .await
                .unwrap();

            // `IotaTransactionBlockResponse` does have a custom PartialEq impl which does
            // not match all options filters but is still good to check if both tx does
            // match
            assert_eq!(fullnode_tx, tx);

            assert!(
                match_transaction_block_resp_options(&options, &[fullnode_tx]),
                "fullnode transaction block assertion failed"
            );
            assert!(
                match_transaction_block_resp_options(&options, &[tx]),
                "indexer transaction block assertion failed"
            );
        });
    }

    fn multi_get_transaction_blocks_with_options(options: IotaTransactionBlockResponseOptions) {
        let (runtime, (cluster, pg_store, indexer_client)) =
            get_global_test_cluster_with_read_write_indexer();
        runtime.block_on(async move {
            indexer_wait_for_checkpoint(pg_store, 3).await;

            let fullnode_checkpoints = cluster
                .rpc_client()
                .get_checkpoints(None, Some(3), false)
                .await
                .unwrap();

            let digests = fullnode_checkpoints
                .data
                .into_iter()
                .flat_map(|c| c.transactions)
                .collect::<Vec<TransactionDigest>>();

            let fullnode_txs = cluster
                .rpc_client()
                .multi_get_transaction_blocks(digests.clone(), Some(options.clone()))
                .await
                .unwrap();

            let indexer_txs = indexer_client
                .multi_get_transaction_blocks(digests, Some(options.clone()))
                .await
                .unwrap();

            // `IotaTransactionBlockResponse` does have a custom PartialEq impl which does
            // not match all options filters but is still good to check if both tx does
            // match
            assert_eq!(fullnode_txs, indexer_txs);

            assert!(
                match_transaction_block_resp_options(&options, &fullnode_txs),
                "fullnode multi transaction blocks assertion failed"
            );
            assert!(
                match_transaction_block_resp_options(&options, &indexer_txs),
                "indexer multi transaction blocks assertion failed"
            );
        });
    }

    #[test]
    fn get_checkpoint_by_seq_num() {
        let (runtime, (cluster, pg_store, indexer_client)) =
            get_global_test_cluster_with_read_write_indexer();

        runtime.block_on(async move {
            indexer_wait_for_checkpoint(pg_store, 1).await;

            let fullnode_checkpoint = cluster
                .rpc_client()
                .get_checkpoint(CheckpointId::SequenceNumber(0))
                .await
                .unwrap();

            let indexer_checkpoint = indexer_client
                .get_checkpoint(CheckpointId::SequenceNumber(0))
                .await
                .unwrap();

            assert_eq!(fullnode_checkpoint, indexer_checkpoint);
        })
    }

    #[test]
    fn get_checkpoint_by_seq_num_not_found() {
        let (runtime, (_cluster, pg_store, indexer_client)) =
            get_global_test_cluster_with_read_write_indexer();

        runtime.block_on(async move {
        indexer_wait_for_checkpoint(pg_store, 1).await;

        let result = indexer_client
            .get_checkpoint(CheckpointId::SequenceNumber(100000000000))
            .await;
        assert!(rpc_call_error_msg_matches(
            result,
            r#"{"code":-32603,"message":"Invalid argument with error: `Checkpoint SequenceNumber(100000000000) not found`"}"#,
        ));
    });
    }

    #[test]
    fn get_checkpoint_by_digest() {
        let (runtime, (cluster, pg_store, indexer_client)) =
            get_global_test_cluster_with_read_write_indexer();
        runtime.block_on(async move {
            indexer_wait_for_checkpoint(pg_store, 1).await;

            let fullnode_checkpoint = cluster
                .rpc_client()
                .get_checkpoint(CheckpointId::SequenceNumber(0))
                .await
                .unwrap();

            let indexer_checkpoint = indexer_client
                .get_checkpoint(CheckpointId::Digest(fullnode_checkpoint.digest))
                .await
                .unwrap();

            assert_eq!(fullnode_checkpoint, indexer_checkpoint);
        });
    }

    #[test]
    fn get_checkpoint_by_digest_not_found() {
        let (runtime, (_cluster, pg_store, indexer_client)) =
            get_global_test_cluster_with_read_write_indexer();

        runtime.block_on(async move {
        indexer_wait_for_checkpoint(pg_store, 1).await;

        let result = indexer_client
            .get_checkpoint(CheckpointId::Digest([0; 32].into()))
            .await;

        assert!(rpc_call_error_msg_matches(
            result,
            r#"{"code":-32603,"message":"Invalid argument with error: `Checkpoint Digest(CheckpointDigest(11111111111111111111111111111111)) not found`"}"#,
        ));
    });
    }

    #[test]
    fn get_checkpoints_all_ascending() {
        let (runtime, (_cluster, pg_store, indexer_client)) =
            get_global_test_cluster_with_read_write_indexer();

        runtime.block_on(async move {
            indexer_wait_for_checkpoint(pg_store, 3).await;

            let indexer_checkpoint = indexer_client
                .get_checkpoints(None, None, false)
                .await
                .unwrap();

            let seq_numbers = indexer_checkpoint
                .data
                .iter()
                .map(|c| c.sequence_number)
                .collect::<Vec<u64>>();

            assert!(is_ascending(&seq_numbers));
        });
    }

    #[test]
    fn get_checkpoints_all_descending() {
        let (runtime, (_cluster, pg_store, indexer_client)) =
            get_global_test_cluster_with_read_write_indexer();
        runtime.block_on(async move {
            indexer_wait_for_checkpoint(pg_store, 3).await;

            let indexer_checkpoint = indexer_client
                .get_checkpoints(None, None, true)
                .await
                .unwrap();

            let seq_numbers = indexer_checkpoint
                .data
                .iter()
                .map(|c| c.sequence_number)
                .collect::<Vec<u64>>();

            assert!(is_descending(&seq_numbers));
        });
    }

    #[test]
    fn get_checkpoints_by_cursor_and_limit_one_descending() {
        let (runtime, (_cluster, pg_store, indexer_client)) =
            get_global_test_cluster_with_read_write_indexer();
        runtime.block_on(async move {
            indexer_wait_for_checkpoint(pg_store, 3).await;

            let indexer_checkpoint = indexer_client
                .get_checkpoints(Some(1.into()), Some(1), true)
                .await
                .unwrap();

            assert_eq!(
                vec![0],
                indexer_checkpoint
                    .data
                    .into_iter()
                    .map(|c| c.sequence_number)
                    .collect::<Vec<u64>>()
            );
        });
    }

    #[test]
    fn get_checkpoints_by_cursor_and_limit_one_ascending() {
        let (runtime, (_cluster, pg_store, indexer_client)) =
            get_global_test_cluster_with_read_write_indexer();
        runtime.block_on(async move {
            indexer_wait_for_checkpoint(pg_store, 3).await;

            let indexer_checkpoint = indexer_client
                .get_checkpoints(Some(1.into()), Some(1), false)
                .await
                .unwrap();

            assert_eq!(
                vec![2],
                indexer_checkpoint
                    .data
                    .into_iter()
                    .map(|c| c.sequence_number)
                    .collect::<Vec<u64>>()
            );
        });
    }

    #[test]
    fn get_checkpoints_by_cursor_zero_and_limit_ascending() {
        let (runtime, (_cluster, pg_store, indexer_client)) =
            get_global_test_cluster_with_read_write_indexer();
        runtime.block_on(async move {
            indexer_wait_for_checkpoint(pg_store, 3).await;

            let indexer_checkpoint = indexer_client
                .get_checkpoints(Some(0.into()), Some(3), false)
                .await
                .unwrap();

            assert_eq!(
                vec![1, 2, 3],
                indexer_checkpoint
                    .data
                    .into_iter()
                    .map(|c| c.sequence_number)
                    .collect::<Vec<u64>>()
            );
        });
    }

    #[test]
    fn get_checkpoints_by_cursor_zero_and_limit_descending() {
        let (runtime, (_cluster, pg_store, indexer_client)) =
            get_global_test_cluster_with_read_write_indexer();
        runtime.block_on(async move {
            indexer_wait_for_checkpoint(pg_store, 3).await;

            let indexer_checkpoint = indexer_client
                .get_checkpoints(Some(0.into()), Some(3), true)
                .await
                .unwrap();

            assert_eq!(
                Vec::<u64>::default(),
                indexer_checkpoint
                    .data
                    .into_iter()
                    .map(|c| c.sequence_number)
                    .collect::<Vec<u64>>()
            );
        });
    }

    #[test]
    fn get_checkpoints_by_cursor_and_limit_ascending() {
        let (runtime, (_cluster, pg_store, indexer_client)) =
            get_global_test_cluster_with_read_write_indexer();
        runtime.block_on(async move {
            indexer_wait_for_checkpoint(pg_store, 6).await;

            let indexer_checkpoint = indexer_client
                .get_checkpoints(Some(3.into()), Some(3), false)
                .await
                .unwrap();

            assert_eq!(
                vec![4, 5, 6],
                indexer_checkpoint
                    .data
                    .into_iter()
                    .map(|c| c.sequence_number)
                    .collect::<Vec<u64>>()
            );
        });
    }

    #[test]
    fn get_checkpoints_by_cursor_and_limit_descending() {
        let (runtime, (_cluster, pg_store, indexer_client)) =
            get_global_test_cluster_with_read_write_indexer();

        runtime.block_on(async move {
            indexer_wait_for_checkpoint(pg_store, 3).await;

            let indexer_checkpoint = indexer_client
                .get_checkpoints(Some(3.into()), Some(3), true)
                .await
                .unwrap();

            assert_eq!(
                // vec![2, 1, 5],
                vec![2, 1, 0],
                indexer_checkpoint
                    .data
                    .into_iter()
                    .map(|c| c.sequence_number)
                    .collect::<Vec<u64>>()
            );
        });
    }

    #[test]
    fn get_checkpoints_invalid_limit() {
        let (runtime, (_cluster, pg_store, indexer_client)) =
            get_global_test_cluster_with_read_write_indexer();

        runtime.block_on(async move {
            indexer_wait_for_checkpoint(pg_store, 3).await;

            let result = indexer_client.get_checkpoints(None, Some(0), false).await;

            assert!(rpc_call_error_msg_matches(
                result,
                r#"{"code":-32602,"message":"Page size limit cannot be smaller than 1"}"#,
            ));
        });
    }

    #[test]
    fn get_object() {
        let (runtime, (cluster, pg_store, indexer_client)) =
            get_global_test_cluster_with_read_write_indexer();
        runtime.block_on(async move {
            indexer_wait_for_checkpoint(pg_store, 1).await;
            let address = cluster.get_address_0();

            let fullnode_objects = cluster
                .rpc_client()
                .get_owned_objects(address, None, None, None)
                .await
                .unwrap();

            for obj in fullnode_objects.data {
                let indexer_obj = indexer_client
                    .get_object(obj.object_id().unwrap(), None)
                    .await
                    .unwrap();
                assert_eq!(obj, indexer_obj)
            }
        });
    }

    #[test]
    fn get_object_not_found() {
        let (runtime, (_cluster, pg_store, indexer_client)) =
            get_global_test_cluster_with_read_write_indexer();
        runtime.block_on(async move {
            indexer_wait_for_checkpoint(pg_store, 1).await;

            let indexer_obj = indexer_client
                .get_object(
                    ObjectID::from_str(
                        "0x9a934a2644c4ca2decbe3d126d80720429c5e31896aa756765afa23ae2cb4b99",
                    )
                    .unwrap(),
                    None,
                )
                .await
                .unwrap();

            assert_eq!(
                indexer_obj,
                IotaObjectResponse {
                    data: None,
                    error: Some(IotaObjectResponseError::NotExists {
                        object_id:
                            "0x9a934a2644c4ca2decbe3d126d80720429c5e31896aa756765afa23ae2cb4b99"
                                .parse()
                                .unwrap()
                    })
                }
            )
        });
    }

    #[test]
    fn get_object_with_bcs_lossless() {
        get_object_with_options(IotaObjectDataOptions::bcs_lossless());
    }

    #[test]
    fn get_object_with_full_content() {
        get_object_with_options(IotaObjectDataOptions::full_content());
    }

    #[test]
    fn get_object_with_bcs() {
        get_object_with_options(IotaObjectDataOptions::default().with_bcs());
    }

    #[test]
    fn get_object_with_content() {
        get_object_with_options(IotaObjectDataOptions::default().with_content());
    }

    #[test]
    fn get_object_with_display() {
        get_object_with_options(IotaObjectDataOptions::default().with_display());
    }

    #[test]
    fn get_object_with_owner() {
        get_object_with_options(IotaObjectDataOptions::default().with_owner());
    }

    #[test]
    fn get_object_with_previous_transaction() {
        get_object_with_options(IotaObjectDataOptions::default().with_previous_transaction());
    }

    #[test]
    fn get_object_with_type() {
        get_object_with_options(IotaObjectDataOptions::default().with_type());
    }

    #[test]
    fn get_object_with_storage_rebate() {
        get_object_with_options(IotaObjectDataOptions {
            show_storage_rebate: true,
            ..Default::default()
        });
    }

    #[test]
    fn multi_get_objects() {
        let (runtime, (cluster, pg_store, indexer_client)) =
            get_global_test_cluster_with_read_write_indexer();
        runtime.block_on(async move {
            indexer_wait_for_checkpoint(pg_store, 1).await;
            let address = cluster.get_address_0();

            let fullnode_objects = cluster
                .rpc_client()
                .get_owned_objects(address, None, None, None)
                .await
                .unwrap();

            let object_ids = fullnode_objects
                .data
                .iter()
                .map(|iota_object| iota_object.object_id().unwrap())
                .collect();

            let indexer_objects = indexer_client
                .multi_get_objects(object_ids, None)
                .await
                .unwrap();

            assert_eq!(fullnode_objects.data, indexer_objects);
        });
    }

    #[test]
    fn multi_get_objects_not_found() {
        let (runtime, (_cluster, pg_store, indexer_client)) =
            get_global_test_cluster_with_read_write_indexer();
        runtime.block_on(async move {
            indexer_wait_for_checkpoint(pg_store, 1).await;

            let object_ids = vec![
                ObjectID::from_str(
                    "0x9a934a2644c4ca2decbe3d126d80720429c5e31896aa756765afa23ae2cb4b99",
                )
                .unwrap(),
                ObjectID::from_str(
                    "0x1a934a7644c4cf2decbe3d126d80720429c5e30896aa756765afa23af3cb4b82",
                )
                .unwrap(),
            ];

            let indexer_objects = indexer_client
                .multi_get_objects(object_ids, None)
                .await
                .unwrap();

            assert_eq!(
                indexer_objects,
                vec![
                    IotaObjectResponse {
                        data: None,
                        error: Some(IotaObjectResponseError::NotExists {
                            object_id:
                                "0x9a934a2644c4ca2decbe3d126d80720429c5e31896aa756765afa23ae2cb4b99"
                                    .parse()
                                    .unwrap()
                        })
                    },
                    IotaObjectResponse {
                        data: None,
                        error: Some(IotaObjectResponseError::NotExists {
                            object_id:
                                "0x1a934a7644c4cf2decbe3d126d80720429c5e30896aa756765afa23af3cb4b82"
                                    .parse()
                                    .unwrap()
                        })
                    }
                ]
            )
        });
    }

    #[test]
    fn multi_get_objects_found_and_not_found() {
        let (runtime, (cluster, pg_store, indexer_client)) =
            get_global_test_cluster_with_read_write_indexer();
        runtime.block_on(async move {
            indexer_wait_for_checkpoint(pg_store, 1).await;
            let address = cluster.get_address_0();

            let fullnode_objects = cluster
                .rpc_client()
                .get_owned_objects(address, None, None, None)
                .await
                .unwrap();

            let mut object_ids = fullnode_objects
                .data
                .iter()
                .map(|iota_object| iota_object.object_id().unwrap())
                .collect::<Vec<ObjectID>>();

            object_ids.extend_from_slice(&[
                ObjectID::from_str(
                    "0x9a934a2644c4ca2decbe3d126d80720429c5e31896aa756765afa23ae2cb4b99",
                )
                .unwrap(),
                ObjectID::from_str(
                    "0x1a934a7644c4cf2decbe3d126d80720429c5e30896aa756765afa23af3cb4b82",
                )
                .unwrap(),
            ]);

            let indexer_objects = indexer_client
                .multi_get_objects(object_ids, None)
                .await
                .unwrap();

            let obj_found_num = indexer_objects
                .iter()
                .filter(|obj_response| obj_response.data.is_some())
                .count();

            assert_eq!(5, obj_found_num);

            let obj_not_found_num = indexer_objects
                .iter()
                .filter(|obj_response| obj_response.error.is_some())
                .count();

            assert_eq!(2, obj_not_found_num);
        });
    }

    #[test]
    fn multi_get_objects_with_bcs_lossless() {
        multi_get_objects_with_options(IotaObjectDataOptions::bcs_lossless());
    }

    #[test]
    fn multi_get_objects_with_full_content() {
        multi_get_objects_with_options(IotaObjectDataOptions::full_content());
    }

    #[test]
    fn multi_get_objects_with_bcs() {
        multi_get_objects_with_options(IotaObjectDataOptions::default().with_bcs());
    }

    #[test]
    fn multi_get_objects_with_content() {
        multi_get_objects_with_options(IotaObjectDataOptions::default().with_content());
    }

    #[test]
    fn multi_get_objects_with_display() {
        multi_get_objects_with_options(IotaObjectDataOptions::default().with_display());
    }

    #[test]
    fn multi_get_objects_with_owner() {
        multi_get_objects_with_options(IotaObjectDataOptions::default().with_owner());
    }

    #[test]
    fn multi_get_objects_with_previous_transaction() {
        multi_get_objects_with_options(
            IotaObjectDataOptions::default().with_previous_transaction(),
        );
    }

    #[test]
    fn multi_get_objects_with_type() {
        multi_get_objects_with_options(IotaObjectDataOptions::default().with_type());
    }

    #[test]
    fn multi_get_objects_with_storage_rebate() {
        multi_get_objects_with_options(IotaObjectDataOptions {
            show_storage_rebate: true,
            ..Default::default()
        });
    }

    #[test]
    fn get_events() {
        let (runtime, (cluster, pg_store, indexer_client)) =
            get_global_test_cluster_with_read_write_indexer();
        runtime.block_on(async move {
            indexer_wait_for_checkpoint(pg_store, 1).await;

            let fullnode_checkpoint = cluster
                .rpc_client()
                .get_checkpoint(CheckpointId::SequenceNumber(0))
                .await
                .unwrap();

            let events = indexer_client
                .get_events(*fullnode_checkpoint.transactions.first().unwrap())
                .await
                .unwrap();

            assert!(!events.is_empty());
        });
    }

    #[test]
    fn get_events_not_found() {
        let (runtime, (_cluster, pg_store, indexer_client)) =
            get_global_test_cluster_with_read_write_indexer();
        runtime.block_on(async move {
        indexer_wait_for_checkpoint(pg_store, 1).await;

        let result = indexer_client.get_events(TransactionDigest::ZERO).await;

        assert!(rpc_call_error_msg_matches(
            result,
            r#"{"code":-32603,"message":"Indexer failed to read PostgresDB with error: `Record not found`"}"#,
        ))
    });
    }

    #[test]
    fn get_transaction_block() {
        let (runtime, (cluster, pg_store, indexer_client)) =
            get_global_test_cluster_with_read_write_indexer();
        runtime.block_on(async move {
            indexer_wait_for_checkpoint(pg_store, 1).await;

            let fullnode_checkpoint = cluster
                .rpc_client()
                .get_checkpoint(CheckpointId::SequenceNumber(0))
                .await
                .unwrap();

            let tx_digest = *fullnode_checkpoint.transactions.first().unwrap();

            let tx = indexer_client
                .get_transaction_block(tx_digest, None)
                .await
                .unwrap();

            assert_eq!(tx_digest, tx.digest);
        });
    }

    #[test]
    fn get_transaction_block_not_found() {
        let (runtime, (_cluster, pg_store, indexer_client)) =
            get_global_test_cluster_with_read_write_indexer();
        runtime.block_on(async move {
        indexer_wait_for_checkpoint(pg_store, 1).await;

        let result = indexer_client
            .get_transaction_block(TransactionDigest::ZERO, None)
            .await;

        assert!(rpc_call_error_msg_matches(
            result,
            r#"{"code":-32603,"message":"Invalid argument with error: `Transaction 11111111111111111111111111111111 not found`"}"#,
        ));
    });
    }

    #[test]
    fn get_transaction_block_with_full_content() {
        get_transaction_block_with_options(IotaTransactionBlockResponseOptions::full_content());
    }

    #[test]
    fn get_transaction_block_with_full_content_and_with_raw_effects() {
        get_transaction_block_with_options(
            IotaTransactionBlockResponseOptions::full_content().with_raw_effects(),
        );
    }

    #[test]
    fn get_transaction_block_with_raw_input() {
        get_transaction_block_with_options(
            IotaTransactionBlockResponseOptions::default().with_raw_input(),
        );
    }

    #[test]
    fn get_transaction_block_with_effects() {
        get_transaction_block_with_options(
            IotaTransactionBlockResponseOptions::default().with_effects(),
        );
    }

    #[test]
    fn get_transaction_block_with_events() {
        get_transaction_block_with_options(
            IotaTransactionBlockResponseOptions::default().with_events(),
        );
    }

    #[test]
    fn get_transaction_block_with_balance_changes() {
        get_transaction_block_with_options(
            IotaTransactionBlockResponseOptions::default().with_balance_changes(),
        );
    }

    #[test]
    fn get_transaction_block_with_object_changes() {
        get_transaction_block_with_options(
            IotaTransactionBlockResponseOptions::default().with_object_changes(),
        );
    }

    #[test]
    fn get_transaction_block_with_raw_effects() {
        get_transaction_block_with_options(
            IotaTransactionBlockResponseOptions::default().with_raw_effects(),
        );
    }

    #[test]
    fn get_transaction_block_with_input() {
        get_transaction_block_with_options(
            IotaTransactionBlockResponseOptions::default().with_input(),
        );
    }

    #[test]
    fn multi_get_transaction_blocks() {
        let (runtime, (cluster, pg_store, indexer_client)) =
            get_global_test_cluster_with_read_write_indexer();
        runtime.block_on(async move {
            indexer_wait_for_checkpoint(pg_store, 3).await;

            let fullnode_checkpoints = cluster
                .rpc_client()
                .get_checkpoints(None, Some(3), false)
                .await
                .unwrap();

            let digests = fullnode_checkpoints
                .data
                .into_iter()
                .flat_map(|c| c.transactions)
                .collect::<Vec<TransactionDigest>>();

            let fullnode_txs = cluster
                .rpc_client()
                .multi_get_transaction_blocks(digests.clone(), None)
                .await
                .unwrap();

            let indexer_txs = indexer_client
                .multi_get_transaction_blocks(digests, None)
                .await
                .unwrap();

            assert_eq!(fullnode_txs, indexer_txs);
        });
    }

    #[test]
    fn multi_get_transaction_blocks_with_full_content() {
        multi_get_transaction_blocks_with_options(
            IotaTransactionBlockResponseOptions::full_content(),
        );
    }

    #[test]
    fn multi_get_transaction_blocks_with_full_content_and_with_raw_effects() {
        multi_get_transaction_blocks_with_options(
            IotaTransactionBlockResponseOptions::full_content().with_raw_effects(),
        );
    }

    #[test]
    fn multi_get_transaction_blocks_with_raw_input() {
        multi_get_transaction_blocks_with_options(
            IotaTransactionBlockResponseOptions::default().with_raw_input(),
        );
    }

    #[test]
    fn multi_get_transaction_blocks_with_effects() {
        multi_get_transaction_blocks_with_options(
            IotaTransactionBlockResponseOptions::default().with_effects(),
        );
    }

    #[test]
    fn multi_get_transaction_blocks_with_events() {
        multi_get_transaction_blocks_with_options(
            IotaTransactionBlockResponseOptions::default().with_events(),
        );
    }

    #[test]
    fn multi_get_transaction_blocks_with_balance_changes() {
        multi_get_transaction_blocks_with_options(
            IotaTransactionBlockResponseOptions::default().with_balance_changes(),
        );
    }

    #[test]
    fn multi_get_transaction_blocks_with_object_changes() {
        multi_get_transaction_blocks_with_options(
            IotaTransactionBlockResponseOptions::default().with_object_changes(),
        );
    }

    #[test]
    fn multi_get_transaction_blocks_with_raw_effects() {
        multi_get_transaction_blocks_with_options(
            IotaTransactionBlockResponseOptions::default().with_raw_effects(),
        );
    }

    #[test]
    fn multi_get_transaction_blocks_with_input() {
        multi_get_transaction_blocks_with_options(
            IotaTransactionBlockResponseOptions::default().with_input(),
        );
    }

    #[test]
    fn get_protocol_config() {
        let (runtime, (cluster, pg_store, indexer_client)) =
            get_global_test_cluster_with_read_write_indexer();
        runtime.block_on(async move {
            indexer_wait_for_checkpoint(pg_store, 1).await;

            let fullnode_protocol_config = cluster
                .rpc_client()
                .get_protocol_config(None)
                .await
                .unwrap();

            let indexer_protocol_config = indexer_client.get_protocol_config(None).await.unwrap();

            assert_eq!(fullnode_protocol_config, indexer_protocol_config);

            let indexer_protocol_config = indexer_client
                .get_protocol_config(Some(1u64.into()))
                .await
                .unwrap();

            assert_eq!(fullnode_protocol_config, indexer_protocol_config);
        });
    }

    #[test]
    fn get_protocol_config_invalid_protocol_version() {
        let (runtime, (_cluster, pg_store, indexer_client)) =
            get_global_test_cluster_with_read_write_indexer();
        runtime.block_on(async move {
        indexer_wait_for_checkpoint(pg_store, 1).await;

        let result = indexer_client
            .get_protocol_config(Some(100u64.into()))
            .await;

        assert!(rpc_call_error_msg_matches(
            result,
            r#"{"code":-32603,"message":"Unsupported protocol version requested. Min supported: 1, max supported: 1"}"#,
        ));
    });
    }

    #[test]
    fn get_chain_identifier() {
        let (runtime, (cluster, pg_store, indexer_client)) =
            get_global_test_cluster_with_read_write_indexer();
        runtime.block_on(async move {
            indexer_wait_for_checkpoint(pg_store, 1).await;

            let fullnode_chain_identifier =
                cluster.rpc_client().get_chain_identifier().await.unwrap();

            let indexer_chain_identifier = indexer_client.get_chain_identifier().await.unwrap();

            assert_eq!(fullnode_chain_identifier, indexer_chain_identifier)
        });
    }

    // #[test]
    // fn get_total_transaction_blocks() {
    //     let stop_after_checkpoint_seq = 5;
    //     let (cluster, pg_store, indexer_client) =
    //         start_test_cluster_with_read_write_indexer(Some(stop_after_checkpoint_seq)).await;

    //     let run_with_range = cluster
    //         .wait_for_run_with_range_shutdown_signal()
    //         .await
    //         .unwrap();

    //     assert!(matches!(
    //             run_with_range,
    //         RunWithRange::Checkpoint(checkpoint_seq_num) if checkpoint_seq_num ==
    // stop_after_checkpoint_seq     ));

    //     // ensure the highest synced checkpoint matches
    //     assert!(cluster.fullnode_handle.iota_node.with(|node| {
    //         node.state()
    //             .get_checkpoint_store()
    //             .get_highest_executed_checkpoint_seq_number()
    //             .unwrap()
    //             == Some(stop_after_checkpoint_seq)
    //     }));

    //     let checkpoint = cluster
    //         .fullnode_handle
    //         .iota_node
    //         .with(|node| {
    //             node.state()
    //                 .get_checkpoint_store()
    //                 .get_checkpoint_by_sequence_number(stop_after_checkpoint_seq)
    //                 .unwrap()
    //         })
    //         .unwrap();

    //     indexer_wait_for_checkpoint(pg_store, stop_after_checkpoint_seq).await;

    //     let total_transaction_blocks = indexer_client
    //         .get_total_transaction_blocks()
    //         .await
    //         .unwrap()
    //         .into_inner();

    //     assert_eq!(
    //         checkpoint.network_total_transactions,
    //         total_transaction_blocks
    //     );
    // }

    // #[test]
    // fn get_latest_checkpoint_sequence_number() {
    //     let stop_after_checkpoint_seq = 5;
    //     let (cluster, pg_store, indexer_client) =
    //         start_test_cluster_with_read_write_indexer(Some(stop_after_checkpoint_seq)).await;

    //     let run_with_range = cluster
    //         .wait_for_run_with_range_shutdown_signal()
    //         .await
    //         .unwrap();

    //     assert!(matches!(
    //             run_with_range,
    //         RunWithRange::Checkpoint(checkpoint_seq_num) if checkpoint_seq_num ==
    // stop_after_checkpoint_seq     ));

    //     // ensure the highest synced checkpoint matches
    //     assert!(cluster.fullnode_handle.iota_node.with(|node| {
    //         node.state()
    //             .get_checkpoint_store()
    //             .get_highest_executed_checkpoint_seq_number()
    //             .unwrap()
    //             == Some(stop_after_checkpoint_seq)
    //     }));

    //     let fullnode_latest_checkpoint_seq_number = cluster
    //         .rpc_client()
    //         .get_latest_checkpoint_sequence_number()
    //         .await
    //         .unwrap()
    //         .into_inner();

    //     indexer_wait_for_checkpoint(pg_store, stop_after_checkpoint_seq).await;

    //     let latest_checkpoint_seq_number = indexer_client
    //         .get_latest_checkpoint_sequence_number()
    //         .await
    //         .unwrap()
    //         .into_inner();

    //     assert!(
    //         (stop_after_checkpoint_seq == latest_checkpoint_seq_number)
    //             && (stop_after_checkpoint_seq ==
    // fullnode_latest_checkpoint_seq_number)     );
    // }

    #[test]
    fn try_get_past_object() {
        let (runtime, (_cluster, pg_store, indexer_client)) =
            get_global_test_cluster_with_read_write_indexer();
        runtime.block_on(async move {
            indexer_wait_for_checkpoint(pg_store, 1).await;

            let result = indexer_client
                .try_get_past_object(ObjectID::random(), SequenceNumber::new(), None)
                .await;
            assert!(rpc_call_error_msg_matches(
                result,
                r#"{"code":-32601,"message":"Method not found"}"#
            ));
        });
    }

    #[test]
    fn try_multi_get_past_objects() {
        let (runtime, (_cluster, pg_store, indexer_client)) =
            get_global_test_cluster_with_read_write_indexer();
        runtime.block_on(async move {
            indexer_wait_for_checkpoint(pg_store, 1).await;

            let result = indexer_client
                .try_multi_get_past_objects(
                    vec![IotaGetPastObjectRequest {
                        object_id: ObjectID::random(),
                        version: SequenceNumber::new(),
                    }],
                    None,
                )
                .await;
            assert!(rpc_call_error_msg_matches(
                result,
                r#"{"code":-32601,"message":"Method not found"}"#
            ));
        });
    }

    #[test]
    fn get_loaded_child_objects() {
        let (runtime, (_cluster, pg_store, indexer_client)) =
            get_global_test_cluster_with_read_write_indexer();
        runtime.block_on(async move {
            indexer_wait_for_checkpoint(pg_store, 1).await;

            let result = indexer_client
                .get_loaded_child_objects(TransactionDigest::ZERO)
                .await;
            assert!(rpc_call_error_msg_matches(
                result,
                r#"{"code":-32601,"message":"Method not found"}"#
            ));
        });
    }
}
