// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_json_rpc_api::{IndexerApiClient, ReadApiClient, WriteApiClient};
use iota_json_rpc_types::{
    IotaExecutionStatus, IotaObjectDataOptions, IotaObjectResponseQuery,
    IotaTransactionBlockEffectsAPI,
};
use iota_macros::sim_test;
use iota_simulator::fastcrypto::encoding::Base64;
use iota_types::{
    object::Owner, programmable_transaction_builder::ProgrammableTransactionBuilder,
    transaction::TransactionKind,
};
use test_cluster::TestClusterBuilder;

#[sim_test]
async fn test_dev_inspect_transaction_block() -> Result<(), anyhow::Error> {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();
    let address = cluster.get_address_0();
    let other_address = cluster.get_address_1();

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

    let obj = objects
        .clone()
        .first()
        .unwrap()
        .object()
        .unwrap()
        .object_ref();

    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();
        builder.transfer_object(other_address, obj).unwrap();
        builder.finish()
    };
    let kind = TransactionKind::programmable(pt);

    let devinspect_response = http_client
        .dev_inspect_transaction_block(
            address,
            Base64::from_bytes(&bcs::to_bytes(&kind).unwrap()),
            None,
            None,
            None,
        )
        .await?;

    assert_eq!(
        *devinspect_response.effects.status(),
        IotaExecutionStatus::Success
    );
    let tx_effect_obj_reassigned = &devinspect_response
        .effects
        .mutated()
        .iter()
        .find(|o| o.reference.object_id == obj.0)
        .unwrap();
    assert_eq!(
        tx_effect_obj_reassigned.owner,
        Owner::AddressOwner(other_address)
    );

    let actual_object_info = http_client
        .get_object(
            obj.0,
            Some(IotaObjectDataOptions {
                show_owner: true,
                ..Default::default()
            }),
        )
        .await
        .unwrap();

    assert_eq!(
        actual_object_info.data.unwrap().owner.unwrap(),
        Owner::AddressOwner(address)
    );

    Ok(())
}
