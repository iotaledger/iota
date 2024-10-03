// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_json_rpc_api::GovernanceReadApiClient;
use iota_json_rpc_types::DelegatedStake;
use iota_types::iota_system_state::iota_system_state_summary::IotaSystemStateSummary;

use crate::common::{indexer_wait_for_checkpoint, rpc_call_error_msg_matches, ApiTestSetup};

#[test]
fn get_stakes_by_id() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        cluster,
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

        let response: Vec<DelegatedStake> = client.get_stakes(validator_address).await.unwrap();
        let stake_id = response
            .first()
            .unwrap()
            .stakes
            .first()
            .unwrap()
            .staked_iota_id;

        let response = client.get_stakes_by_ids(vec![stake_id]).await.unwrap();

        assert_eq!(response.len(), 1);
    });
}

#[test]
fn get_stakes() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        cluster,
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

        let response = client.get_stakes(validator_address).await.unwrap();

        assert_eq!(response.len(), 1);
    });
}

#[test]
fn get_timelocked_stakes_by_id() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        ..
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        indexer_wait_for_checkpoint(store, 1).await;

        let response = client.get_timelocked_stakes_by_ids(vec![]).await.unwrap();

        assert_eq!(response.len(), 0);
    });
}

#[test]
fn get_timelocked_stakes() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        cluster,
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
        let indexer_response = client.get_committee_info(Some(0.into())).await.unwrap();

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
fn get_reference_gas_price() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        ..
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        indexer_wait_for_checkpoint(store, 1).await;

        let response = client.get_reference_gas_price().await.unwrap();
        println!("{:?}", response)
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

        let response: IotaSystemStateSummary = client.get_latest_iota_system_state().await.unwrap();
        println!("{:?}", response)
    });
}

#[test]
fn get_validators_apy() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        ..
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        indexer_wait_for_checkpoint(store, 1).await;

        let response = client.get_validators_apy().await.unwrap();
        let (apys, epoch) = (response.apys, response.epoch);

        assert_eq!(epoch, 0);
        assert_eq!(apys.len(), 4);
        assert_eq!(apys.iter().find(|apy| apy.apy == 0.0).is_some(), true);
    });
}
