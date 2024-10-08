// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{path::Path, str::FromStr, time::Duration};

use iota_json_rpc_api::{
    IndexerApiClient, ReadApiClient, TransactionBuilderClient, WriteApiClient,
};
use iota_json_rpc_types::{
    CheckpointId, IotaGetPastObjectRequest, IotaObjectDataOptions, IotaObjectResponse,
    IotaObjectResponseQuery, IotaPastObjectResponse, IotaTransactionBlockEffectsAPI,
    IotaTransactionBlockResponse, IotaTransactionBlockResponseOptions, ObjectChange,
    ProtocolConfigResponse, TransactionBlockBytes,
};
use iota_macros::sim_test;
use iota_move_build::BuildConfig;
use iota_types::{
    base_types::{ObjectID, SequenceNumber},
    digests::TransactionDigest,
    error::IotaObjectResponseError,
    messages_checkpoint::CheckpointSequenceNumber,
    quorum_driver_types::ExecuteTransactionRequestType,
    transaction::CallArg,
    IOTA_FRAMEWORK_ADDRESS,
};
use test_cluster::{TestCluster, TestClusterBuilder};

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

/// Checks if
/// [`iota_json_rpc_types::IotaObjectResponse`] match to the
/// provided
/// [`iota_json_rpc_types::IotaObjectDataOptions`] filters
fn match_object_resp_options(
    expected_options: &IotaObjectDataOptions,
    responses: &[IotaObjectResponse],
) -> bool {
    responses
        .iter()
        .map(|iota_obj_resp| {
            let obj_data = iota_obj_resp.data.as_ref().unwrap();

            IotaObjectDataOptions {
                show_type: obj_data.type_.is_some(),
                show_owner: obj_data.owner.is_some(),
                show_previous_transaction: obj_data.previous_transaction.is_some(),
                show_display: obj_data.display.is_some(),
                show_content: obj_data.content.is_some(),
                show_bcs: obj_data.bcs.is_some(),
                show_storage_rebate: obj_data.storage_rebate.is_some(),
            }
        })
        .all(|actual_options| actual_options.eq(expected_options))
}

/// Checks if
/// [`iota_json_rpc_types::IotaPastObjectResponse`] match to the
/// provided
/// [`iota_json_rpc_types::IotaObjectDataOptions`] filters
fn match_past_object_resp_options(
    expected_options: &IotaObjectDataOptions,
    responses: &[IotaPastObjectResponse],
) -> bool {
    assert!(!responses.is_empty());
    responses
        .iter()
        .map(|iota_past_obj_resp| match iota_past_obj_resp {
            IotaPastObjectResponse::VersionFound(iota_object_data) => IotaObjectDataOptions {
                show_type: iota_object_data.type_.is_some(),
                show_owner: iota_object_data.owner.is_some(),
                show_previous_transaction: iota_object_data.previous_transaction.is_some(),
                show_display: iota_object_data.display.is_some(),
                show_content: iota_object_data.content.is_some(),
                show_bcs: iota_object_data.bcs.is_some(),
                show_storage_rebate: iota_object_data.storage_rebate.is_some(),
            },
            _ => unreachable!(),
        })
        .all(|actual_options| actual_options.eq(expected_options))
}

/// Wait for the `TestCluster` to catch up to the given checkpoint sequence
/// number
pub async fn wait_for_checkpoint(cluster: &TestCluster, checkpoint_sequence_number: u64) {
    tokio::time::timeout(Duration::from_secs(60), async {
        loop {
            let fullnode_checkpoint = cluster
                .fullnode_handle
                .iota_node
                .with(|node| {
                    node.state()
                        .get_checkpoint_store()
                        .get_highest_executed_checkpoint_seq_number()
                })
                .unwrap();

            match fullnode_checkpoint {
                Some(c) if c >= checkpoint_sequence_number => break,
                _ => tokio::time::sleep(Duration::from_millis(100)).await,
            }
        }
    })
    .await
    .expect("Timeout waiting for indexer to catchup to checkpoint");
}

async fn create_transactions(
    cluster: &TestCluster,
    options: IotaTransactionBlockResponseOptions,
) -> Vec<IotaTransactionBlockResponse> {
    let http_client = cluster.rpc_client();
    let address = cluster.get_address_0();

    let objects = http_client
        .get_owned_objects(address, None, None, None)
        .await
        .unwrap()
        .data;

    let gas_id = objects.last().unwrap().object().unwrap().object_id;

    // Make some transactions
    let mut tx_responses: Vec<IotaTransactionBlockResponse> = Vec::new();

    for obj in objects.iter().take(objects.len() - 1) {
        let oref = obj.object().unwrap();

        let transaction_bytes: TransactionBlockBytes = http_client
            .transfer_object(
                address,
                oref.object_id,
                Some(gas_id),
                1_000_000.into(),
                address,
            )
            .await
            .unwrap();

        let tx = cluster
            .wallet
            .sign_transaction(&transaction_bytes.to_data().unwrap());

        let (tx_bytes, signatures) = tx.to_tx_bytes_and_signatures();

        let response = http_client
            .execute_transaction_block(
                tx_bytes,
                signatures,
                Some(options.clone()),
                Some(ExecuteTransactionRequestType::WaitForLocalExecution),
            )
            .await
            .unwrap();

        tx_responses.push(response);
    }

    tx_responses
}

async fn get_owned_objects(
    cluster: &TestCluster,
    options: IotaObjectDataOptions,
) -> Vec<IotaObjectResponse> {
    let http_client = cluster.rpc_client();
    let address = cluster.get_address_0();

    http_client
        .get_owned_objects(
            address,
            Some(IotaObjectResponseQuery::new_with_options(options)),
            None,
            None,
        )
        .await
        .unwrap()
        .data
}

async fn get_transaction_block_with_options(options: IotaTransactionBlockResponseOptions) {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();

    let tx_response = create_transactions(&cluster, options.clone()).await;

    assert!(match_transaction_block_resp_options(&options, &tx_response));

    for tx_digest in tx_response
        .iter()
        .map(|a| a.digest)
        .collect::<Vec<TransactionDigest>>()
    {
        let transaction_block = http_client
            .get_transaction_block(tx_digest, Some(options.clone()))
            .await
            .unwrap();

        assert!(match_transaction_block_resp_options(
            &options,
            &[transaction_block]
        ));
    }
}

async fn multi_get_transaction_blocks_with_options(options: IotaTransactionBlockResponseOptions) {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();

    let tx_response = create_transactions(&cluster, options.clone()).await;

    assert!(match_transaction_block_resp_options(&options, &tx_response));

    let digests = tx_response
        .iter()
        .map(|tx_block_resp| tx_block_resp.digest)
        .collect::<Vec<TransactionDigest>>();

    let transaction_blocks = http_client
        .multi_get_transaction_blocks(digests, Some(options.clone()))
        .await
        .unwrap();

    assert!(match_transaction_block_resp_options(
        &options,
        &transaction_blocks
    ));
}

async fn get_object_with_options(options: IotaObjectDataOptions) {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();

    let objects = get_owned_objects(&cluster, options.clone()).await;

    for obj in objects {
        let oref = obj.into_object().unwrap();
        let result = http_client
            .get_object(oref.object_id, Some(options.clone()))
            .await
            .unwrap();

        assert!(match_object_resp_options(&options, &[result]));
    }
}
async fn multi_get_objects_with_options(options: IotaObjectDataOptions) {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();

    let objects = get_owned_objects(&cluster, options.clone()).await;

    let object_ids = objects
        .iter()
        .map(|obj| obj.data.as_ref().unwrap().object_id)
        .collect::<Vec<ObjectID>>();

    let response = http_client
        .multi_get_objects(object_ids, Some(options.clone()))
        .await
        .unwrap();

    assert!(match_object_resp_options(&options, &response))
}

async fn try_get_past_object_with_options(options: IotaObjectDataOptions) {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();
    let address = cluster.get_address_0();

    let fullnode_objects = http_client
        .get_owned_objects(address, None, None, None)
        .await
        .unwrap()
        .data;

    for object in fullnode_objects.iter() {
        let seq_num = object.data.as_ref().unwrap().version;
        let object_id = object.object_id().unwrap();

        let rpc_past_obj = http_client
            .try_get_past_object(object_id, seq_num, Some(options.clone()))
            .await
            .unwrap();

        assert!(
            matches!(rpc_past_obj, IotaPastObjectResponse::VersionFound(ref obj) if obj.object_id == object_id && obj.version == seq_num)
        );
        assert!(match_past_object_resp_options(&options, &[rpc_past_obj]));
    }

    let tx_response = create_transactions(
        &cluster,
        IotaTransactionBlockResponseOptions::default().with_object_changes(),
    )
    .await;

    let mutated_objects = tx_response
        .iter()
        .flat_map(|tx_block_res| {
            tx_block_res.object_changes.as_ref().map(|obj_changes| {
                obj_changes
                    .iter()
                    .filter_map(|obj_change| {
                        if let ObjectChange::Mutated {
                            object_id, version, ..
                        } = obj_change
                        {
                            return Some((object_id, version));
                        };
                        None
                    })
                    .collect::<Vec<(&ObjectID, &SequenceNumber)>>()
            })
        })
        .flatten()
        .collect::<Vec<_>>();

    for (mutated_obj_id, mutated_obj_version) in mutated_objects {
        let rpc_past_obj = http_client
            .try_get_past_object(*mutated_obj_id, *mutated_obj_version, Some(options.clone()))
            .await
            .unwrap();

        assert!(
            matches!(rpc_past_obj, IotaPastObjectResponse::VersionFound(ref obj) if obj.object_id ==
    *mutated_obj_id && obj.version == *mutated_obj_version)
        );
        assert!(match_past_object_resp_options(&options, &[rpc_past_obj]));
    }
}
async fn try_multi_get_past_objects_with_options(options: IotaObjectDataOptions) {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();
    let address = cluster.get_address_0();

    let fullnode_objects = http_client
        .get_owned_objects(address, None, None, None)
        .await
        .unwrap()
        .data;

    let past_obj_requests = fullnode_objects
        .iter()
        .map(|obj_res| {
            let obj = obj_res.data.as_ref().unwrap();
            let object_id = obj.object_id;
            let version = obj.version;

            IotaGetPastObjectRequest { object_id, version }
        })
        .collect::<Vec<IotaGetPastObjectRequest>>();

    let rpc_past_objs = http_client
        .try_multi_get_past_objects(past_obj_requests, Some(options.clone()))
        .await
        .unwrap();
    assert!(match_past_object_resp_options(&options, &rpc_past_objs));

    let tx_response = create_transactions(
        &cluster,
        IotaTransactionBlockResponseOptions::default().with_object_changes(),
    )
    .await;

    let mutated_past_objects_req = tx_response
        .iter()
        .flat_map(|tx_block_res| {
            tx_block_res.object_changes.as_ref().map(|obj_changes| {
                obj_changes
                    .iter()
                    .filter_map(|obj_change| {
                        if let ObjectChange::Mutated {
                            object_id, version, ..
                        } = obj_change
                        {
                            return Some(IotaGetPastObjectRequest {
                                object_id: *object_id,
                                version: *version,
                            });
                        };
                        None
                    })
                    .collect::<Vec<IotaGetPastObjectRequest>>()
            })
        })
        .flatten()
        .collect::<Vec<_>>();

    let rpc_past_objs = http_client
        .try_multi_get_past_objects(mutated_past_objects_req, Some(options.clone()))
        .await
        .unwrap();
    assert!(match_past_object_resp_options(&options, &rpc_past_objs));
}

async fn publish_move_package(cluster: &TestCluster) -> IotaTransactionBlockResponse {
    let http_client = cluster.rpc_client();
    let address = cluster.get_address_0();

    let objects = http_client
        .get_owned_objects(
            address,
            Some(IotaObjectResponseQuery::new_with_options(
                IotaObjectDataOptions::new()
                    .with_type()
                    .with_owner()
                    .with_previous_transaction(),
            )),
            None,
            None,
        )
        .await
        .unwrap();
    let gas = objects.data.first().unwrap().object().unwrap();

    let compiled_package = BuildConfig::new_for_testing()
        .build(Path::new("../../examples/move/basics"))
        .unwrap();
    let compiled_modules_bytes =
        compiled_package.get_package_base64(/* with_unpublished_deps */ false);
    let dependencies = compiled_package.get_dependency_storage_package_ids();

    let transaction_bytes: TransactionBlockBytes = http_client
        .publish(
            address,
            compiled_modules_bytes,
            dependencies,
            Some(gas.object_id),
            100_000_000.into(),
        )
        .await
        .unwrap();

    let tx = cluster
        .wallet
        .sign_transaction(&transaction_bytes.to_data().unwrap());
    let (tx_bytes, signatures) = tx.to_tx_bytes_and_signatures();

    let tx_response: IotaTransactionBlockResponse = http_client
        .execute_transaction_block(
            tx_bytes,
            signatures,
            Some(IotaTransactionBlockResponseOptions::full_content()),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await
        .unwrap();

    matches!(tx_response, IotaTransactionBlockResponse {ref effects, ..} if effects.as_ref().unwrap().created().len() == 6);
    tx_response
}

#[tokio::test]
async fn test_get_package_with_display_should_not_fail() -> Result<(), anyhow::Error> {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();

    let response = http_client
        .get_object(
            ObjectID::from(IOTA_FRAMEWORK_ADDRESS),
            Some(IotaObjectDataOptions::new().with_display()),
        )
        .await;
    assert!(response.is_ok());
    let response: IotaObjectResponse = response?;
    assert!(response
        .into_object()
        .unwrap()
        .display
        .unwrap()
        .data
        .is_none());
    Ok(())
}

#[sim_test]
async fn test_get_object_info() -> Result<(), anyhow::Error> {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();
    let address = cluster.get_address_0();

    let objects = http_client
        .get_owned_objects(
            address,
            Some(IotaObjectResponseQuery::new_with_options(
                IotaObjectDataOptions::new()
                    .with_type()
                    .with_owner()
                    .with_previous_transaction(),
            )),
            None,
            None,
        )
        .await?
        .data;

    for obj in objects {
        let oref = obj.into_object().unwrap();
        let rpc_obj = http_client
            .get_object(
                oref.object_id,
                Some(IotaObjectDataOptions::new().with_owner()),
            )
            .await?;
        assert!(
            matches!(rpc_obj, IotaObjectResponse { data: Some(object), .. } if oref.object_id == object.object_id && object.owner.unwrap().get_owner_address()? == address)
        );
    }
    Ok(())
}

#[sim_test]
async fn test_get_objects() -> Result<(), anyhow::Error> {
    let cluster = TestClusterBuilder::new().build().await;

    let http_client = cluster.rpc_client();
    let address = cluster.get_address_0();

    let objects = http_client
        .get_owned_objects(
            address,
            Some(IotaObjectResponseQuery::new_with_options(
                IotaObjectDataOptions::new(),
            )),
            None,
            None,
        )
        .await?;
    assert_eq!(5, objects.data.len());

    // Multiget objectIDs test
    let object_digests = objects
        .data
        .iter()
        .map(|o| o.object().unwrap().object_id)
        .collect();

    let object_resp = http_client.multi_get_objects(object_digests, None).await?;
    assert_eq!(5, object_resp.len());
    Ok(())
}

#[sim_test]
async fn get_object_not_found() {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();

    let rpc_obj_resp = http_client
        .get_object(
            ObjectID::from_str(
                "0x9a934a2644c4ca2decbe3d126d80720429c5e31896aa756765afa23ae2cb4b99",
            )
            .unwrap(),
            None,
        )
        .await
        .unwrap();

    assert!(matches!(rpc_obj_resp, IotaObjectResponse {  error, .. } if error.is_some()));
}

#[sim_test]
async fn get_transaction_block() {
    get_transaction_block_with_options(IotaTransactionBlockResponseOptions::default()).await;
}

#[sim_test]
async fn get_transaction_block_with_full_content() {
    get_transaction_block_with_options(IotaTransactionBlockResponseOptions::full_content()).await;
}

#[ignore = "until this will be merged into develop https://github.com/iotaledger/iota/blob/sc-platform/indexer-new-rpc-tests/crates/iota-json-rpc/src/read_api.rs#L1351"]
#[sim_test]
async fn get_transaction_block_with_full_content_and_with_raw_effects() {
    get_transaction_block_with_options(
        IotaTransactionBlockResponseOptions::full_content().with_raw_effects(),
    )
    .await;
}

#[sim_test]
async fn get_transaction_block_with_raw_input() {
    get_transaction_block_with_options(
        IotaTransactionBlockResponseOptions::default().with_raw_input(),
    )
    .await;
}

#[sim_test]
async fn get_transaction_block_with_effects() {
    get_transaction_block_with_options(
        IotaTransactionBlockResponseOptions::default().with_effects(),
    )
    .await;
}

#[sim_test]
async fn get_transaction_block_with_events() {
    get_transaction_block_with_options(
        IotaTransactionBlockResponseOptions::default().with_events(),
    )
    .await;
}

#[sim_test]
async fn get_transaction_block_with_balance_changes() {
    get_transaction_block_with_options(
        IotaTransactionBlockResponseOptions::default().with_balance_changes(),
    )
    .await;
}

#[sim_test]
async fn get_transaction_block_with_object_changes() {
    get_transaction_block_with_options(
        IotaTransactionBlockResponseOptions::default().with_object_changes(),
    )
    .await;
}

#[ignore = "until this will be merged into develop https://github.com/iotaledger/iota/blob/sc-platform/indexer-new-rpc-tests/crates/iota-json-rpc/src/read_api.rs#L1351"]
#[sim_test]
async fn get_transaction_block_with_raw_effects() {
    get_transaction_block_with_options(
        IotaTransactionBlockResponseOptions::default().with_raw_effects(),
    )
    .await;
}

#[sim_test]
async fn get_transaction_block_with_input() {
    get_transaction_block_with_options(IotaTransactionBlockResponseOptions::default().with_input())
        .await;
}

#[sim_test]
async fn multi_get_transaction_blocks() {
    multi_get_transaction_blocks_with_options(IotaTransactionBlockResponseOptions::default()).await;
}

#[ignore = "until this will be merged into develop https://github.com/iotaledger/iota/blob/sc-platform/indexer-new-rpc-tests/crates/iota-json-rpc/src/read_api.rs#L1351"]
#[sim_test]
async fn multi_get_transaction_blocks_with_full_content_and_with_raw_effects() {
    multi_get_transaction_blocks_with_options(
        IotaTransactionBlockResponseOptions::full_content().with_raw_effects(),
    )
    .await;
}

#[sim_test]
async fn multi_get_transaction_blocks_with_raw_input() {
    multi_get_transaction_blocks_with_options(
        IotaTransactionBlockResponseOptions::default().with_raw_input(),
    )
    .await;
}

#[sim_test]
async fn multi_get_transaction_blocks_with_effects() {
    multi_get_transaction_blocks_with_options(
        IotaTransactionBlockResponseOptions::default().with_effects(),
    )
    .await;
}

#[sim_test]
async fn multi_get_transaction_blocks_with_events() {
    multi_get_transaction_blocks_with_options(
        IotaTransactionBlockResponseOptions::default().with_events(),
    )
    .await;
}

#[sim_test]
async fn multi_get_transaction_blocks_with_balance_changes() {
    multi_get_transaction_blocks_with_options(
        IotaTransactionBlockResponseOptions::default().with_balance_changes(),
    )
    .await;
}

#[sim_test]
async fn multi_get_transaction_blocks_with_object_changes() {
    multi_get_transaction_blocks_with_options(
        IotaTransactionBlockResponseOptions::default().with_object_changes(),
    )
    .await;
}

#[ignore = "until this will be merged into develop https://github.com/iotaledger/iota/blob/sc-platform/indexer-new-rpc-tests/crates/iota-json-rpc/src/read_api.rs#L1351"]
#[sim_test]
async fn multi_get_transaction_blocks_with_raw_effects() {
    multi_get_transaction_blocks_with_options(
        IotaTransactionBlockResponseOptions::default().with_raw_effects(),
    )
    .await;
}

#[sim_test]
async fn multi_get_transaction_blocks_with_input() {
    multi_get_transaction_blocks_with_options(
        IotaTransactionBlockResponseOptions::default().with_input(),
    )
    .await;
}

#[sim_test]
async fn get_object() {
    get_object_with_options(IotaObjectDataOptions::default()).await;
}

#[sim_test]
async fn get_object_with_bcs_lossless() {
    get_object_with_options(IotaObjectDataOptions::bcs_lossless()).await;
}

#[sim_test]
async fn get_object_with_full_content() {
    get_object_with_options(IotaObjectDataOptions::full_content()).await;
}

#[sim_test]
async fn get_object_with_bcs() {
    get_object_with_options(IotaObjectDataOptions::default().with_bcs()).await;
}

#[sim_test]
async fn get_object_with_content() {
    get_object_with_options(IotaObjectDataOptions::default().with_content()).await;
}

#[sim_test]
async fn get_object_with_display() {
    get_object_with_options(IotaObjectDataOptions::default().with_display()).await;
}

#[sim_test]
async fn get_object_with_owner() {
    get_object_with_options(IotaObjectDataOptions::default().with_owner()).await;
}

#[sim_test]
async fn get_object_with_previous_transaction() {
    get_object_with_options(IotaObjectDataOptions::default().with_previous_transaction()).await;
}

#[sim_test]
async fn get_object_with_type() {
    get_object_with_options(IotaObjectDataOptions::default().with_type()).await;
}

#[sim_test]
async fn get_object_with_storage_rebate() {
    get_object_with_options(IotaObjectDataOptions {
        show_storage_rebate: true,
        ..Default::default()
    })
    .await;
}

#[sim_test]
async fn multi_get_objects_not_found() {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();

    let object_ids = vec![
        ObjectID::from_str("0x9a934a2644c4ca2decbe3d126d80720429c5e31896aa756765afa23ae2cb4b99")
            .unwrap(),
        ObjectID::from_str("0x1a934a7644c4cf2decbe3d126d80720429c5e30896aa756765afa23af3cb4b82")
            .unwrap(),
    ];

    let indexer_objects = http_client
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
    );
}

#[sim_test]
async fn multi_get_objects() {
    multi_get_objects_with_options(IotaObjectDataOptions::default()).await;
}

#[sim_test]
async fn multi_get_objects_with_bcs_lossless() {
    multi_get_objects_with_options(IotaObjectDataOptions::bcs_lossless()).await;
}

#[sim_test]
async fn multi_get_objects_with_full_content() {
    multi_get_objects_with_options(IotaObjectDataOptions::full_content()).await;
}

#[sim_test]
async fn multi_get_objects_with_bcs() {
    multi_get_objects_with_options(IotaObjectDataOptions::default().with_bcs()).await;
}

#[sim_test]
async fn multi_get_objects_with_content() {
    multi_get_objects_with_options(IotaObjectDataOptions::default().with_content()).await;
}

#[sim_test]
async fn multi_get_objects_with_display() {
    multi_get_objects_with_options(IotaObjectDataOptions::default().with_display()).await;
}

#[sim_test]
async fn multi_get_objects_with_owner() {
    multi_get_objects_with_options(IotaObjectDataOptions::default().with_owner()).await;
}

#[sim_test]
async fn multi_get_objects_with_previous_transaction() {
    multi_get_objects_with_options(IotaObjectDataOptions::default().with_previous_transaction())
        .await;
}

#[sim_test]
async fn multi_get_objects_with_type() {
    multi_get_objects_with_options(IotaObjectDataOptions::default().with_type()).await;
}

#[sim_test]
async fn multi_get_objects_with_storage_rebate() {
    multi_get_objects_with_options(IotaObjectDataOptions {
        show_storage_rebate: true,
        ..Default::default()
    })
    .await;
}

#[sim_test]
async fn get_checkpoint_by_seq_number() {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();

    wait_for_checkpoint(&cluster, 6).await;

    let fullnode_checkpoints = cluster
        .fullnode_handle
        .iota_node
        .with_async(|node| async {
            node.state()
                .get_checkpoint_store()
                .multi_get_checkpoint_by_sequence_number(
                    &(0..=5).collect::<Vec<CheckpointSequenceNumber>>(),
                )
                .unwrap()
        })
        .await;

    for checkpoint in fullnode_checkpoints {
        let envelope = checkpoint.unwrap();
        let digest = *envelope.digest();
        let checkpoint_summary = envelope.into_inner().into_data();

        let rpc_checkpoint = http_client
            .get_checkpoint(CheckpointId::SequenceNumber(
                checkpoint_summary.sequence_number,
            ))
            .await
            .unwrap();

        assert_eq!(rpc_checkpoint.epoch, checkpoint_summary.epoch);
        assert_eq!(
            rpc_checkpoint.sequence_number,
            checkpoint_summary.sequence_number
        );
        assert_eq!(
            rpc_checkpoint.network_total_transactions,
            checkpoint_summary.network_total_transactions
        );
        assert_eq!(rpc_checkpoint.digest, digest);
        assert_eq!(
            rpc_checkpoint.previous_digest,
            checkpoint_summary.previous_digest
        );
        assert_eq!(
            rpc_checkpoint.epoch_rolling_gas_cost_summary,
            checkpoint_summary.epoch_rolling_gas_cost_summary
        );
        assert_eq!(rpc_checkpoint.timestamp_ms, checkpoint_summary.timestamp_ms);
        assert_eq!(
            rpc_checkpoint.end_of_epoch_data,
            checkpoint_summary.end_of_epoch_data
        );
    }
}

#[sim_test]
async fn get_checkpoint_by_seq_number_not_found() {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();

    let result = http_client
        .get_checkpoint(CheckpointId::SequenceNumber(86734872426827346))
        .await;

    assert!(result.is_err())
}

#[sim_test]
async fn get_checkpoint_by_digest() {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();

    wait_for_checkpoint(&cluster, 6).await;

    let fullnode_checkpoints = cluster
        .fullnode_handle
        .iota_node
        .with_async(|node| async {
            node.state()
                .get_checkpoint_store()
                .multi_get_checkpoint_by_sequence_number(
                    &(0..=5).collect::<Vec<CheckpointSequenceNumber>>(),
                )
                .unwrap()
        })
        .await;

    for checkpoint in fullnode_checkpoints {
        let envelope = checkpoint.unwrap();
        let digest = *envelope.digest();
        let checkpoint_summary = envelope.into_inner().into_data();

        let rpc_checkpoint = http_client
            .get_checkpoint(CheckpointId::Digest(digest))
            .await
            .unwrap();

        assert_eq!(rpc_checkpoint.epoch, checkpoint_summary.epoch);
        assert_eq!(
            rpc_checkpoint.sequence_number,
            checkpoint_summary.sequence_number
        );
        assert_eq!(
            rpc_checkpoint.network_total_transactions,
            checkpoint_summary.network_total_transactions
        );
        assert_eq!(rpc_checkpoint.digest, digest);
        assert_eq!(
            rpc_checkpoint.previous_digest,
            checkpoint_summary.previous_digest
        );
        assert_eq!(
            rpc_checkpoint.epoch_rolling_gas_cost_summary,
            checkpoint_summary.epoch_rolling_gas_cost_summary
        );
        assert_eq!(rpc_checkpoint.timestamp_ms, checkpoint_summary.timestamp_ms);
        assert_eq!(
            rpc_checkpoint.end_of_epoch_data,
            checkpoint_summary.end_of_epoch_data
        );
    }
}

#[sim_test]
async fn get_checkpoint_by_digest_not_found() {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();

    let result = http_client
        .get_checkpoint(CheckpointId::Digest([0; 32].into()))
        .await;

    assert!(result.is_err())
}

#[sim_test]
async fn get_checkpoints_all_ascending() {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();

    wait_for_checkpoint(&cluster, 3).await;

    let rpc_checkpoints = http_client
        .get_checkpoints(None, None, false)
        .await
        .unwrap();

    let checkpoint_seq_ids = rpc_checkpoints
        .data
        .iter()
        .map(|checkpoint| checkpoint.sequence_number)
        .collect::<Vec<u64>>();

    assert!(is_ascending(&checkpoint_seq_ids))
}

#[sim_test]
async fn get_checkpoints_all_descending() {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();

    wait_for_checkpoint(&cluster, 3).await;

    let rpc_checkpoints = http_client.get_checkpoints(None, None, true).await.unwrap();

    let checkpoint_seq_ids = rpc_checkpoints
        .data
        .iter()
        .map(|checkpoint| checkpoint.sequence_number)
        .collect::<Vec<u64>>();

    assert!(is_descending(&checkpoint_seq_ids))
}

#[sim_test]
async fn get_checkpoints_by_cursor_and_limit_one_descending() {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();

    wait_for_checkpoint(&cluster, 3).await;

    let rpc_checkpoints = http_client
        .get_checkpoints(Some(1.into()), Some(1), true)
        .await
        .unwrap();

    let checkpoint_seq_ids = rpc_checkpoints
        .data
        .iter()
        .map(|checkpoint| checkpoint.sequence_number)
        .collect::<Vec<u64>>();

    assert_eq!(vec![0], checkpoint_seq_ids)
}

#[sim_test]
async fn get_checkpoints_by_cursor_and_limit_one_ascending() {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();

    wait_for_checkpoint(&cluster, 3).await;

    let rpc_checkpoints = http_client
        .get_checkpoints(Some(1.into()), Some(1), false)
        .await
        .unwrap();

    let checkpoint_seq_ids = rpc_checkpoints
        .data
        .iter()
        .map(|checkpoint| checkpoint.sequence_number)
        .collect::<Vec<u64>>();

    assert_eq!(vec![2], checkpoint_seq_ids)
}

#[sim_test]
async fn get_checkpoints_by_cursor_zero_and_limit_ascending() {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();

    wait_for_checkpoint(&cluster, 3).await;

    let rpc_checkpoints = http_client
        .get_checkpoints(Some(0.into()), Some(3), false)
        .await
        .unwrap();

    let checkpoint_seq_ids = rpc_checkpoints
        .data
        .iter()
        .map(|checkpoint| checkpoint.sequence_number)
        .collect::<Vec<u64>>();

    assert_eq!(vec![1, 2, 3], checkpoint_seq_ids)
}

#[sim_test]
async fn get_checkpoints_by_cursor_zero_and_limit_descending() {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();

    wait_for_checkpoint(&cluster, 3).await;

    let rpc_checkpoints = http_client
        .get_checkpoints(Some(0.into()), Some(3), true)
        .await
        .unwrap();

    let checkpoint_seq_ids = rpc_checkpoints
        .data
        .iter()
        .map(|checkpoint| checkpoint.sequence_number)
        .collect::<Vec<u64>>();

    assert_eq!(vec![0], checkpoint_seq_ids)
}

#[sim_test]
async fn get_checkpoints_by_cursor_and_limit_ascending() {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();

    wait_for_checkpoint(&cluster, 6).await;

    let rpc_checkpoints = http_client
        .get_checkpoints(Some(3.into()), Some(3), false)
        .await
        .unwrap();

    let checkpoint_seq_ids = rpc_checkpoints
        .data
        .iter()
        .map(|checkpoint| checkpoint.sequence_number)
        .collect::<Vec<u64>>();

    assert_eq!(vec![4, 5, 6], checkpoint_seq_ids)
}

#[sim_test]
async fn get_checkpoints_by_cursor_and_limit_descending() {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();

    wait_for_checkpoint(&cluster, 3).await;

    let rpc_checkpoints = http_client
        .get_checkpoints(Some(3.into()), Some(3), true)
        .await
        .unwrap();

    let checkpoint_seq_ids = rpc_checkpoints
        .data
        .iter()
        .map(|checkpoint| checkpoint.sequence_number)
        .collect::<Vec<u64>>();

    assert_eq!(vec![2, 1, 0], checkpoint_seq_ids)
}

#[sim_test]
async fn get_checkpoints_invalid_limit() {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();

    let result = http_client.get_checkpoints(None, Some(0), false).await;

    assert!(result.is_err())
}

#[sim_test]
async fn get_events() {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();

    let fullnode_checkpoint = cluster
        .rpc_client()
        .get_checkpoint(CheckpointId::SequenceNumber(0))
        .await
        .unwrap();

    let rpc_events = http_client
        .get_events(*fullnode_checkpoint.transactions.first().unwrap())
        .await
        .unwrap();

    assert!(!rpc_events.is_empty())
}

#[sim_test]
async fn get_events_not_found() {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();

    let result = http_client.get_events(TransactionDigest::ZERO).await;

    assert!(result.is_err())
}

#[sim_test]
async fn get_total_transaction_blocks() {
    let checkpoint_seq_num = 3;

    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();

    wait_for_checkpoint(&cluster, checkpoint_seq_num).await;

    let fullnode_checkpoint = cluster
        .rpc_client()
        .get_checkpoint(CheckpointId::SequenceNumber(checkpoint_seq_num))
        .await
        .unwrap();

    let rpc_total_transaction_blocks = http_client.get_total_transaction_blocks().await.unwrap();

    assert!(
        rpc_total_transaction_blocks.into_inner() >= fullnode_checkpoint.network_total_transactions
    )
}

#[sim_test]
async fn get_latest_checkpoint_sequence_number() {
    let checkpoint_seq_num = 3;

    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();

    wait_for_checkpoint(&cluster, checkpoint_seq_num).await;

    let fullnode_checkpoint = cluster
        .rpc_client()
        .get_checkpoint(CheckpointId::SequenceNumber(checkpoint_seq_num))
        .await
        .unwrap();

    let rpc_latest_checkpoint_sequence_number = http_client
        .get_latest_checkpoint_sequence_number()
        .await
        .unwrap();

    assert!(
        rpc_latest_checkpoint_sequence_number.into_inner() >= fullnode_checkpoint.sequence_number
    )
}

#[sim_test]
async fn get_protocol_config() {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();

    let fullnode_protocol_config = cluster.fullnode_handle.iota_node.with(|node| {
        node.state()
            .epoch_store_for_testing()
            .protocol_config()
            .clone()
    });

    let fullnode_protocol_config = ProtocolConfigResponse::from(fullnode_protocol_config);

    let rpc_protocol_config = http_client.get_protocol_config(None).await.unwrap();

    assert_eq!(rpc_protocol_config, fullnode_protocol_config);

    let rpc_protocol_config = http_client
        .get_protocol_config(Some(
            fullnode_protocol_config.protocol_version.as_u64().into(),
        ))
        .await
        .unwrap();

    assert_eq!(rpc_protocol_config, fullnode_protocol_config);
}

#[sim_test]
async fn get_protocol_config_invalid_protocol_version() {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();

    let result = http_client
        .get_protocol_config(Some(7826517365.into()))
        .await;

    assert!(result.is_err())
}

#[sim_test]
async fn get_chain_identifier() {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();

    let fullnode_chain_identifier = cluster
        .fullnode_handle
        .iota_node
        .with(|node| node.state().get_chain_identifier())
        .unwrap();

    let rpc_chain_identifier = http_client.get_chain_identifier().await.unwrap();

    assert_eq!(fullnode_chain_identifier.to_string(), rpc_chain_identifier);
}

#[sim_test]
async fn try_get_past_object() {
    try_get_past_object_with_options(IotaObjectDataOptions::default()).await;
}

#[sim_test]
async fn try_get_past_object_with_bcs_lossless() {
    try_get_past_object_with_options(IotaObjectDataOptions::bcs_lossless()).await;
}

#[sim_test]
async fn try_get_past_object_with_full_content() {
    try_get_past_object_with_options(IotaObjectDataOptions::full_content()).await;
}

#[sim_test]
async fn try_get_past_object_with_bcs() {
    try_get_past_object_with_options(IotaObjectDataOptions::default().with_bcs()).await;
}

#[sim_test]
async fn try_get_past_object_with_content() {
    try_get_past_object_with_options(IotaObjectDataOptions::default().with_content()).await;
}

#[sim_test]
async fn try_get_past_object_with_display() {
    try_get_past_object_with_options(IotaObjectDataOptions::default().with_display()).await;
}

#[sim_test]
async fn try_get_past_object_with_owner() {
    try_get_past_object_with_options(IotaObjectDataOptions::default().with_owner()).await;
}

#[sim_test]
async fn try_get_past_object_with_previous_transaction() {
    try_get_past_object_with_options(IotaObjectDataOptions::default().with_previous_transaction())
        .await;
}

#[sim_test]
async fn try_get_past_object_with_type() {
    try_get_past_object_with_options(IotaObjectDataOptions::default().with_type()).await;
}

#[sim_test]
async fn try_get_past_object_with_storage_rebate() {
    try_get_past_object_with_options(IotaObjectDataOptions {
        show_storage_rebate: true,
        ..Default::default()
    })
    .await;
}

#[sim_test]
async fn try_multi_get_past_objects() {
    try_multi_get_past_objects_with_options(IotaObjectDataOptions::default()).await;
}

#[sim_test]
async fn try_multi_get_past_objects_with_bcs_lossless() {
    try_multi_get_past_objects_with_options(IotaObjectDataOptions::bcs_lossless()).await;
}

#[sim_test]
async fn try_multi_get_past_objects_with_full_content() {
    try_multi_get_past_objects_with_options(IotaObjectDataOptions::full_content()).await;
}

#[sim_test]
async fn try_multi_get_past_objects_with_bcs() {
    try_multi_get_past_objects_with_options(IotaObjectDataOptions::default().with_bcs()).await;
}

#[sim_test]
async fn try_multi_get_past_objects_with_content() {
    try_multi_get_past_objects_with_options(IotaObjectDataOptions::default().with_content()).await;
}

#[sim_test]
async fn try_multi_get_past_objects_with_display() {
    try_multi_get_past_objects_with_options(IotaObjectDataOptions::default().with_display()).await;
}

#[sim_test]
async fn try_multi_get_past_objects_with_owner() {
    try_multi_get_past_objects_with_options(IotaObjectDataOptions::default().with_owner()).await;
}

#[sim_test]
async fn try_multi_get_past_objects_with_previous_transaction() {
    try_multi_get_past_objects_with_options(
        IotaObjectDataOptions::default().with_previous_transaction(),
    )
    .await;
}

#[sim_test]
async fn try_multi_get_past_objects_with_type() {
    try_multi_get_past_objects_with_options(IotaObjectDataOptions::default().with_type()).await;
}

#[sim_test]
async fn try_multi_get_past_objects_with_storage_rebate() {
    try_multi_get_past_objects_with_options(IotaObjectDataOptions {
        show_storage_rebate: true,
        ..Default::default()
    })
    .await;
}

#[sim_test]
async fn try_get_past_object_not_exists() {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();

    let rpc_past_obj = http_client
        .try_get_past_object(ObjectID::ZERO, SequenceNumber::from_u64(1), None)
        .await
        .unwrap();

    assert!(
        matches!(rpc_past_obj, IotaPastObjectResponse::ObjectNotExists(ref obj_id) if obj_id == &ObjectID::ZERO)
    );
}

#[sim_test]
async fn try_get_past_object_version_too_high() {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();
    let address = cluster.get_address_0();

    let fullnode_objects = http_client
        .get_owned_objects(address, None, None, None)
        .await
        .unwrap()
        .data;

    let seq_num = SequenceNumber::from_u64(5);
    for object in fullnode_objects.iter() {
        let object_id = object.object_id().unwrap();

        let rpc_past_obj = http_client
            .try_get_past_object(object_id, seq_num, None)
            .await
            .unwrap();

        assert!(
            matches!(rpc_past_obj, IotaPastObjectResponse::VersionTooHigh{object_id: obj_id, asked_version, latest_version} if obj_id == object_id && asked_version == seq_num && latest_version == SequenceNumber::from_u64(1))
        );
    }
}

#[sim_test]
async fn try_get_past_object_version_not_found() {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();

    let tx_response = create_transactions(
        &cluster,
        IotaTransactionBlockResponseOptions::default().with_object_changes(),
    )
    .await;

    let mutated_objects = tx_response
        .iter()
        .flat_map(|tx_block_res| {
            tx_block_res.object_changes.as_ref().map(|obj_changes| {
                obj_changes
                    .iter()
                    .filter_map(|obj_change| {
                        if let ObjectChange::Mutated {
                            object_id, version, ..
                        } = obj_change
                        {
                            if version > &SequenceNumber::from_u64(2) {
                                return Some(object_id);
                            } else {
                                return None;
                            }
                        };
                        None
                    })
                    .collect::<Vec<&ObjectID>>()
            })
        })
        .flatten()
        .collect::<Vec<_>>();

    let seq_num = SequenceNumber::from_u64(2);
    let mut at_least_one_version_not_found = false;

    for mutated_obj_id in mutated_objects {
        if !cluster
            .fullnode_handle
            .iota_node
            .with(|node| {
                node.state()
                    .get_object_cache_reader()
                    .object_exists_by_key(mutated_obj_id, seq_num)
            })
            .unwrap()
        {
            let rpc_past_obj = http_client
                .try_get_past_object(*mutated_obj_id, seq_num, None)
                .await
                .unwrap();

            assert!(
                matches!(rpc_past_obj, IotaPastObjectResponse::VersionNotFound(obj_id, seq_number) if obj_id == *mutated_obj_id && seq_number == seq_num)
            );
            at_least_one_version_not_found = true;
        }
    }
    assert!(at_least_one_version_not_found)
}

#[sim_test]
async fn try_get_past_object_deleted() {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();
    let address = cluster.get_address_0();

    let objects = http_client
        .get_owned_objects(
            address,
            Some(IotaObjectResponseQuery::new_with_options(
                IotaObjectDataOptions::full_content(),
            )),
            None,
            None,
        )
        .await
        .unwrap();

    assert_eq!(5, objects.data.len());

    let tx_block_response = publish_move_package(&cluster).await;

    let package_id = tx_block_response
        .object_changes
        .unwrap()
        .iter()
        .filter_map(|obj_change| match obj_change {
            ObjectChange::Published { package_id, .. } => Some(*package_id),
            _ => None,
        })
        .collect::<Vec<ObjectID>>()[0];

    let tx_block_response = cluster
        .sign_and_execute_transaction(
            &cluster
                .test_transaction_builder()
                .await
                .move_call(
                    package_id,
                    "object_basics",
                    "create",
                    vec![1u64.into(), CallArg::Pure(address.to_vec())],
                )
                .build(),
        )
        .await;

    let created_object_id = tx_block_response
        .object_changes
        .unwrap()
        .iter()
        .filter_map(|obj_change| match obj_change {
            ObjectChange::Created { object_id, .. } => Some(*object_id),
            _ => None,
        })
        .collect::<Vec<ObjectID>>()[0];

    let objects = http_client
        .get_owned_objects(
            address,
            Some(IotaObjectResponseQuery::new_with_options(
                IotaObjectDataOptions::full_content(),
            )),
            None,
            None,
        )
        .await
        .unwrap();

    let object_ids = objects
        .data
        .iter()
        .map(|a| a.object_id().unwrap())
        .collect::<Vec<ObjectID>>();

    assert_eq!(7, objects.data.len());
    assert!(object_ids.contains(&created_object_id));

    let created_object = http_client
        .get_object(created_object_id, None)
        .await
        .unwrap()
        .data
        .unwrap();

    let arg = CallArg::Object(iota_types::transaction::ObjectArg::ImmOrOwnedObject((
        created_object.object_id,
        created_object.version,
        created_object.digest,
    )));

    let tx_block_response = cluster
        .sign_and_execute_transaction(
            &cluster
                .test_transaction_builder()
                .await
                .move_call(package_id, "object_basics", "delete", vec![arg])
                .build(),
        )
        .await;

    assert_eq!(
        tx_block_response.effects.as_ref().unwrap().deleted().len(),
        1
    );

    let seq_num = SequenceNumber::from_u64(4);
    let rpc_past_obj = http_client
        .try_get_past_object(created_object_id, seq_num, None)
        .await
        .unwrap();

    assert!(
        matches!(rpc_past_obj, IotaPastObjectResponse::ObjectDeleted(obj) if obj.object_id == created_object_id && obj.version == seq_num)
    );
}

#[sim_test]
async fn try_get_object_before_version() {
    let options = IotaObjectDataOptions::bcs_lossless();

    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();
    let address = cluster.get_address_0();

    let fullnode_objects = http_client
        .get_owned_objects(address, None, None, None)
        .await
        .unwrap()
        .data;

    for object in fullnode_objects.iter() {
        let seq_num = object.data.as_ref().unwrap().version;
        let object_id = object.object_id().unwrap();

        let rpc_obj_before_ver = http_client
            .try_get_object_before_version(object_id, seq_num)
            .await
            .unwrap();

        assert!(
            matches!(rpc_obj_before_ver, IotaPastObjectResponse::VersionFound(ref obj) if obj.object_id == object_id && obj.version == seq_num)
        );
        assert!(match_past_object_resp_options(
            &options,
            &[rpc_obj_before_ver]
        ));
    }

    let tx_response = create_transactions(
        &cluster,
        IotaTransactionBlockResponseOptions::default().with_object_changes(),
    )
    .await;

    let mutated_objects = tx_response
        .iter()
        .flat_map(|tx_block_res| {
            tx_block_res.object_changes.as_ref().map(|obj_changes| {
                obj_changes
                    .iter()
                    .filter_map(|obj_change| {
                        if let ObjectChange::Mutated {
                            object_id, version, ..
                        } = obj_change
                        {
                            return Some((object_id, version));
                        };
                        None
                    })
                    .collect::<Vec<(&ObjectID, &SequenceNumber)>>()
            })
        })
        .flatten()
        .collect::<Vec<_>>();

    for (mutated_obj_id, mutated_obj_version) in mutated_objects {
        let rpc_obj_before_ver = http_client
            .try_get_object_before_version(*mutated_obj_id, *mutated_obj_version)
            .await
            .unwrap();

        assert!(
            matches!(rpc_obj_before_ver, IotaPastObjectResponse::VersionFound(ref obj) if obj.object_id ==
    *mutated_obj_id && obj.version == *mutated_obj_version)
        );
        assert!(match_past_object_resp_options(
            &options,
            &[rpc_obj_before_ver]
        ));
    }
}

#[sim_test]
async fn try_get_object_before_version_not_exists() {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();

    let rpc_obj_before_ver = http_client
        .try_get_object_before_version(ObjectID::ZERO, SequenceNumber::from_u64(1))
        .await
        .unwrap();

    assert!(
        matches!(rpc_obj_before_ver, IotaPastObjectResponse::ObjectNotExists(ref obj_id) if obj_id == &ObjectID::ZERO)
    );
}
