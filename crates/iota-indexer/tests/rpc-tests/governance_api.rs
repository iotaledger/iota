// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0


use crate::common::{ApiTestSetup, indexer_wait_for_checkpoint, rpc_call_error_msg_matches};
use iota_json_rpc_api::GovernanceReadApiClient;

/**
#[test]
fn get_validators_apy() {
    let mut genesis_config = GenesisConfig::for_local_testing();

    let validator_config = genesis_config.validator_config_info.as_mut().unwrap();

    // Add a different commission rate to each validator
    for (i, validator) in validator_config.iter_mut().enumerate() {
        validator.commission_rate = i as u64;
    }

    let test_cluster = TestClusterBuilder::new()
        .set_genesis_config(genesis_config)
        .with_epoch_duration_ms(10_000)
        .with_num_validators(4)
        .build()
        .await;

    let (pg_store, indexer_client) = start_read_write_indexer_with_existing_test_cluster(&test_cluster).await;

    indexer_wait_for_checkpoint(&pg_store, 1).await;

    let response = indexer_client.get_validators_apy().await.unwrap();
    let (apys, epoch) = (response.apys, response.epoch);

    assert_eq!(epoch, 0);
    assert_eq!(apys.len(), 4);
    assert_eq!(apys.iter().find(|apy| apy.apy == 0.0).is_some(), true)
}
**/

#[test]
fn get_reference_gas_price() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        ..
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        indexer_wait_for_checkpoint(store, 1).await;

        let response = client
            .get_reference_gas_price()
            .await
            .unwrap();
    });

}

#[test]
fn get_latest_iota_system_state() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        ..
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        indexer_wait_for_checkpoint(store, 1).await;

        let response = client
            .get_latest_iota_system_state()
            .await
            .unwrap();
    });

}

#[test]
fn get_committee_info() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        ..
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        indexer_wait_for_checkpoint(store, 1).await;

        // Test with no specified epoch
        let indexer_response = client.get_committee_info(None).await.unwrap();

        let (epoch_id, validators) = (indexer_response.epoch, indexer_response.validators);

        assert!(epoch_id == 0);
        assert_eq!(validators.len(), 4);

        // Test with specified epoch 0
        let indexer_response = client
            .get_committee_info(Some(0.into()))
            .await
            .unwrap();

        let (epoch_id, validators) = (indexer_response.epoch, indexer_response.validators);

        assert!(epoch_id == 0);
        assert_eq!(validators.len(), 4);

        // Test with non-existent epoch
        let response = client.get_committee_info(Some(1.into())).await;

        rpc_call_error_msg_matches(
            response,
            r#"{"code":-32603,"message":"Invalid argument with error: `Missing epoch Some(1)`"}"#,
        );
    });

}

#[test]
fn get_timelocked_stakes() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        cluster
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        indexer_wait_for_checkpoint(store, 1).await;

        let validator_address = cluster
            .swarm
            .active_validators()
            .next()
            .unwrap()
            .config
            .iota_address();

        let response = client
            .get_timelocked_stakes(validator_address)
            .await
            .unwrap();

        assert_eq!(response.len(), 0);
    });

}
