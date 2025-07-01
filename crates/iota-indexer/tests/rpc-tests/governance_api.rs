// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_json_rpc_api::GovernanceReadApiClient;
use iota_protocol_config::ProtocolVersion;
use iota_types::iota_system_state::iota_system_state_summary::IotaSystemStateSummary;

use crate::common::{ApiTestSetup, indexer_wait_for_checkpoint};

#[test]
fn get_latest_iota_system_state_v2() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        cluster,
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        indexer_wait_for_checkpoint(store, 1).await;

        // ensure that the system state is updated
        cluster.force_new_epoch().await;
        let system_state = client.get_latest_iota_system_state_v2().await.unwrap();
        let IotaSystemStateSummary::V2(system_state_v2) = system_state else {
            panic!("expected IotaSystemStateSummaryV2");
        };
        assert_eq!(
            system_state_v2.protocol_version,
            ProtocolVersion::MAX.as_u64()
        );
        assert_eq!(system_state_v2.system_state_version, 2);
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
        let response = client.get_committee_info(None).await.unwrap();

        assert_eq!(response.validators.len(), 4);

        // Test with specified epoch 0
        let response = client.get_committee_info(Some(0.into())).await.unwrap();

        let (epoch_id, validators) = (response.epoch, response.validators);

        assert!(epoch_id == 0);
        assert_eq!(validators.len(), 4);

        // Test with non-existent epoch
        let response = client.get_committee_info(Some(u64::MAX.into())).await;

        assert!(response.is_err());
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
        assert_eq!(response, 1000.into());
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

        let apys = client.get_validators_apy().await.unwrap().apys;

        assert_eq!(apys.len(), 4);
        assert!(apys.iter().any(|apy| apy.apy == 0.0));
    });
}
