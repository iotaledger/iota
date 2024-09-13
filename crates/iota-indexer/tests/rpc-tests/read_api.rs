// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::str::FromStr;

use iota_config::node::RunWithRange;
use iota_json_rpc_api::{IndexerApiClient, ReadApiClient};
use iota_json_rpc_types::{
    CheckpointId, IotaGetPastObjectRequest, IotaObjectDataOptions, IotaObjectResponse,
    IotaObjectResponseQuery, IotaTransactionBlockResponseOptions,
};
use iota_types::{
    base_types::{ObjectID, SequenceNumber},
    digests::TransactionDigest,
    error::IotaObjectResponseError,
};

use crate::common::{
    assert_rpc_call_error_msg, indexer_wait_for_checkpoint,
    start_test_cluster_with_read_write_indexer,
};

fn is_ascending(vec: &[u64]) -> bool {
    vec.windows(2).all(|window| window[0] <= window[1])
}
fn is_descending(vec: &[u64]) -> bool {
    vec.windows(2).all(|window| window[0] >= window[1])
}

#[tokio::test]
async fn test_get_checkpoint() {
    let (cluster, pg_store, indexer_client) =
        start_test_cluster_with_read_write_indexer(None).await;
    // indexer starts storing data after checkpoint 0
    indexer_wait_for_checkpoint(&pg_store, 1).await;

    let fullnode_checkpoint = cluster
        .rpc_client()
        .get_checkpoint(CheckpointId::SequenceNumber(0))
        .await
        .unwrap();

    let checkpoint_indexer = indexer_client
        .get_checkpoint(CheckpointId::SequenceNumber(0))
        .await
        .unwrap();

    assert_eq!(fullnode_checkpoint, checkpoint_indexer);

    let checkpoint_indexer = indexer_client
        .get_checkpoint(CheckpointId::Digest(fullnode_checkpoint.digest))
        .await
        .unwrap();

    assert_eq!(fullnode_checkpoint, checkpoint_indexer);

    let result = indexer_client
        .get_checkpoint(CheckpointId::SequenceNumber(100000000000))
        .await;

    assert_rpc_call_error_msg(
        result,
        r#"{"code":-32603,"message":"Invalid argument with error: `Checkpoint SequenceNumber(100000000000) not found`"}"#,
    );

    let result = indexer_client
        .get_checkpoint(CheckpointId::Digest([0; 32].into()))
        .await;

    assert_rpc_call_error_msg(
        result,
        r#"{"code":-32603,"message":"Invalid argument with error: `Checkpoint Digest(CheckpointDigest(11111111111111111111111111111111)) not found`"}"#,
    );
}

#[tokio::test]
async fn test_get_checkpoints() {
    let (_cluster, pg_store, indexer_client) =
        start_test_cluster_with_read_write_indexer(None).await;
    // indexer starts storing data after checkpoint 0
    indexer_wait_for_checkpoint(&pg_store, 3).await;

    let checkpoint_indexer = indexer_client
        .get_checkpoints(None, None, false)
        .await
        .unwrap();

    let seq_numbers = checkpoint_indexer
        .data
        .iter()
        .map(|c| c.sequence_number)
        .collect::<Vec<u64>>();

    assert!(is_ascending(&seq_numbers));

    let checkpoint_indexer = indexer_client
        .get_checkpoints(None, None, true)
        .await
        .unwrap();

    let seq_numbers = checkpoint_indexer
        .data
        .iter()
        .map(|c| c.sequence_number)
        .collect::<Vec<u64>>();

    assert!(is_descending(&seq_numbers));

    let checkpoint_indexer = indexer_client
        .get_checkpoints(Some(1.into()), Some(1), true)
        .await
        .unwrap();

    assert_eq!(
        vec![0],
        checkpoint_indexer
            .data
            .into_iter()
            .map(|c| c.sequence_number)
            .collect::<Vec<u64>>()
    );

    let checkpoint_indexer = indexer_client
        .get_checkpoints(Some(0.into()), Some(3), false)
        .await
        .unwrap();

    assert_eq!(
        vec![1, 2, 3],
        checkpoint_indexer
            .data
            .into_iter()
            .map(|c| c.sequence_number)
            .collect::<Vec<u64>>()
    );

    let checkpoint_indexer = indexer_client
        .get_checkpoints(Some(0.into()), Some(3), true)
        .await
        .unwrap();

    assert_eq!(
        Vec::<u64>::default(),
        checkpoint_indexer
            .data
            .into_iter()
            .map(|c| c.sequence_number)
            .collect::<Vec<u64>>()
    );

    let result = indexer_client.get_checkpoints(None, Some(0), false).await;

    assert_rpc_call_error_msg(
        result,
        r#"{"code":-32602,"message":"Page size limit cannot be smaller than 1"}"#,
    );
}

#[tokio::test]
async fn test_get_object() {
    let (cluster, pg_store, indexer_client) =
        start_test_cluster_with_read_write_indexer(None).await;
    // indexer starts storing data after checkpoint 0
    indexer_wait_for_checkpoint(&pg_store, 1).await;
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

    let fullnode_objects = cluster
        .rpc_client()
        .get_owned_objects(
            address,
            Some(IotaObjectResponseQuery::new_with_options(
                IotaObjectDataOptions::full_content().with_bcs(),
            )),
            None,
            None,
        )
        .await
        .unwrap();

    for obj in fullnode_objects.data {
        let indexer_obj = indexer_client
            .get_object(
                obj.object_id().unwrap(),
                Some(IotaObjectDataOptions::full_content().with_bcs()),
            )
            .await
            .unwrap();
        assert_eq!(obj, indexer_obj)
    }

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
                object_id: "0x9a934a2644c4ca2decbe3d126d80720429c5e31896aa756765afa23ae2cb4b99"
                    .parse()
                    .unwrap()
            })
        }
    )
}

#[tokio::test]
async fn test_multi_get_objects() {
    let (cluster, pg_store, indexer_client) =
        start_test_cluster_with_read_write_indexer(None).await;
    // indexer starts storing data after checkpoint 0
    indexer_wait_for_checkpoint(&pg_store, 1).await;
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

    let fullnode_objects = cluster
        .rpc_client()
        .get_owned_objects(
            address,
            Some(IotaObjectResponseQuery::new_with_options(
                IotaObjectDataOptions::full_content().with_bcs(),
            )),
            None,
            None,
        )
        .await
        .unwrap();

    let object_ids = fullnode_objects
        .data
        .iter()
        .map(|iota_object| iota_object.object_id().unwrap())
        .collect();

    let indexer_objects = indexer_client
        .multi_get_objects(
            object_ids,
            Some(IotaObjectDataOptions::full_content().with_bcs()),
        )
        .await
        .unwrap();

    assert_eq!(fullnode_objects.data, indexer_objects);

    let object_ids = vec![
        ObjectID::from_str("0x9a934a2644c4ca2decbe3d126d80720429c5e31896aa756765afa23ae2cb4b99")
            .unwrap(),
        ObjectID::from_str("0x1a934a7644c4cf2decbe3d126d80720429c5e30896aa756765afa23af3cb4b82")
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
                    object_id: "0x9a934a2644c4ca2decbe3d126d80720429c5e31896aa756765afa23ae2cb4b99"
                        .parse()
                        .unwrap()
                })
            },
            IotaObjectResponse {
                data: None,
                error: Some(IotaObjectResponseError::NotExists {
                    object_id: "0x1a934a7644c4cf2decbe3d126d80720429c5e30896aa756765afa23af3cb4b82"
                        .parse()
                        .unwrap()
                })
            }
        ]
    )
}

#[tokio::test]
async fn test_get_events() {
    let (cluster, pg_store, indexer_client) =
        start_test_cluster_with_read_write_indexer(None).await;
    // indexer starts storing data after checkpoint 0
    indexer_wait_for_checkpoint(&pg_store, 1).await;

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

    let result = indexer_client.get_events(TransactionDigest::ZERO).await;

    assert_rpc_call_error_msg(
        result,
        r#"{"code":-32603,"message":"Could not find the referenced transaction events [TransactionEventsDigest(11111111111111111111111111111111)]."}"#,
    );
}

#[tokio::test]
async fn test_get_transaction_block() {
    let (cluster, pg_store, indexer_client) =
        start_test_cluster_with_read_write_indexer(None).await;
    // indexer starts storing data after checkpoint 0
    indexer_wait_for_checkpoint(&pg_store, 1).await;

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

    let result = indexer_client
        .get_transaction_block(TransactionDigest::ZERO, None)
        .await;

    assert_rpc_call_error_msg(
        result,
        r#"{"code":-32603,"message":"Invalid argument with error: `Transaction 11111111111111111111111111111111 not found`"}"#,
    );

    let fullnode_tx = cluster
        .rpc_client()
        .get_transaction_block(
            tx_digest,
            Some(IotaTransactionBlockResponseOptions::full_content().with_raw_effects()),
        )
        .await
        .unwrap();

    let tx = indexer_client
        .get_transaction_block(
            tx_digest,
            Some(IotaTransactionBlockResponseOptions::full_content().with_raw_effects()),
        )
        .await
        .unwrap();

    assert_eq!(fullnode_tx, tx)
}

#[tokio::test]
async fn test_multi_get_transaction_blocks() {
    let (cluster, pg_store, indexer_client) =
        start_test_cluster_with_read_write_indexer(None).await;
    // indexer starts storing data after checkpoint 0
    indexer_wait_for_checkpoint(&pg_store, 3).await;

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
}

#[tokio::test]
async fn test_get_protocol_config() {
    let (cluster, pg_store, indexer_client) =
        start_test_cluster_with_read_write_indexer(None).await;
    // indexer starts storing data after checkpoint 0
    indexer_wait_for_checkpoint(&pg_store, 1).await;

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

    let result = indexer_client
        .get_protocol_config(Some(100u64.into()))
        .await;

    assert_rpc_call_error_msg(
        result,
        r#"{"code":-32603,"message":"Unsupported protocol version requested. Min supported: 1, max supported: 1"}"#,
    );
}

#[tokio::test]
async fn test_get_chain_identifier() {
    let (cluster, pg_store, indexer_client) =
        start_test_cluster_with_read_write_indexer(None).await;
    // indexer starts storing data after checkpoint 0
    indexer_wait_for_checkpoint(&pg_store, 1).await;

    let fullnode_chain_identifier = cluster.rpc_client().get_chain_identifier().await.unwrap();

    let indexer_chain_identifier = indexer_client.get_chain_identifier().await.unwrap();

    assert_eq!(fullnode_chain_identifier, indexer_chain_identifier)
}

#[tokio::test]
async fn test_get_total_transaction_blocks() {
    let stop_after_checkpoint_seq = 5;
    let (cluster, pg_store, indexer_client) =
        start_test_cluster_with_read_write_indexer(Some(stop_after_checkpoint_seq)).await;

    let run_with_range = cluster
        .wait_for_run_with_range_shutdown_signal()
        .await
        .unwrap();

    assert!(matches!(
        run_with_range,
        RunWithRange::Checkpoint(checkpoint_seq_num) if checkpoint_seq_num == stop_after_checkpoint_seq
    ));

    // ensure the highest synced checkpoint matches
    assert!(cluster.fullnode_handle.iota_node.with(|node| {
        node.state()
            .get_checkpoint_store()
            .get_highest_executed_checkpoint_seq_number()
            .unwrap()
            == Some(stop_after_checkpoint_seq)
    }));

    let checkpoint = cluster
        .fullnode_handle
        .iota_node
        .with(|node| {
            node.state()
                .get_checkpoint_store()
                .get_checkpoint_by_sequence_number(stop_after_checkpoint_seq)
                .unwrap()
        })
        .unwrap();

    indexer_wait_for_checkpoint(&pg_store, stop_after_checkpoint_seq).await;

    let total_transaction_blocks = indexer_client
        .get_total_transaction_blocks()
        .await
        .unwrap()
        .into_inner();

    assert_eq!(
        checkpoint.network_total_transactions,
        total_transaction_blocks
    );
}

#[tokio::test]
async fn test_get_latest_checkpoint_sequence_number() {
    let stop_after_checkpoint_seq = 5;
    let (cluster, pg_store, indexer_client) =
        start_test_cluster_with_read_write_indexer(Some(stop_after_checkpoint_seq)).await;

    let run_with_range = cluster
        .wait_for_run_with_range_shutdown_signal()
        .await
        .unwrap();

    assert!(matches!(
        run_with_range,
        RunWithRange::Checkpoint(checkpoint_seq_num) if checkpoint_seq_num == stop_after_checkpoint_seq
    ));

    // ensure the highest synced checkpoint matches
    assert!(cluster.fullnode_handle.iota_node.with(|node| {
        node.state()
            .get_checkpoint_store()
            .get_highest_executed_checkpoint_seq_number()
            .unwrap()
            == Some(stop_after_checkpoint_seq)
    }));

    let fullnode_latest_checkpoint_seq_number = cluster
        .rpc_client()
        .get_latest_checkpoint_sequence_number()
        .await
        .unwrap()
        .into_inner();

    // indexer starts storing data after checkpoint 0
    indexer_wait_for_checkpoint(&pg_store, stop_after_checkpoint_seq).await;

    let latest_checkpoint_seq_number = indexer_client
        .get_latest_checkpoint_sequence_number()
        .await
        .unwrap()
        .into_inner();

    assert!(
        (stop_after_checkpoint_seq == latest_checkpoint_seq_number)
            && (stop_after_checkpoint_seq == fullnode_latest_checkpoint_seq_number)
    );
}

#[tokio::test]
async fn test_try_get_past_object() {
    let (_cluster, pg_store, indexer_client) =
        start_test_cluster_with_read_write_indexer(None).await;
    // indexer starts storing data after checkpoint 0
    indexer_wait_for_checkpoint(&pg_store, 1).await;

    let result = indexer_client
        .try_get_past_object(ObjectID::random(), SequenceNumber::new(), None)
        .await;
    assert_rpc_call_error_msg(result, r#"{"code":-32601,"message":"Method not found"}"#);
}

#[tokio::test]
async fn test_try_multi_get_past_objects() {
    let (_cluster, pg_store, indexer_client) =
        start_test_cluster_with_read_write_indexer(None).await;
    // indexer starts storing data after checkpoint 0
    indexer_wait_for_checkpoint(&pg_store, 1).await;

    let result = indexer_client
        .try_multi_get_past_objects(
            vec![IotaGetPastObjectRequest {
                object_id: ObjectID::random(),
                version: SequenceNumber::new(),
            }],
            None,
        )
        .await;
    assert_rpc_call_error_msg(result, r#"{"code":-32601,"message":"Method not found"}"#);
}

#[tokio::test]
async fn test_get_loaded_child_objects() {
    let (_cluster, pg_store, indexer_client) =
        start_test_cluster_with_read_write_indexer(None).await;
    // indexer starts storing data after checkpoint 0
    indexer_wait_for_checkpoint(&pg_store, 1).await;

    let result = indexer_client
        .get_loaded_child_objects(TransactionDigest::ZERO)
        .await;
    assert_rpc_call_error_msg(result, r#"{"code":-32601,"message":"Method not found"}"#);
}
