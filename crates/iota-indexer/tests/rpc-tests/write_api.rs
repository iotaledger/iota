// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use fastcrypto::encoding::Base64;
use iota_json_rpc_api::{ReadApiClient, TransactionBuilderClient, WriteApiClient};
use iota_json_rpc_types::{
    IotaExecutionStatus, IotaObjectDataOptions, IotaTransactionBlockEffectsAPI,
    IotaTransactionBlockResponseOptions,
};
use iota_types::{
    base_types::{IotaAddress, ObjectID},
    crypto::{AccountKeyPair, get_key_pair},
    object::Owner,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    quorum_driver_types::ExecuteTransactionRequestType,
    transaction::TransactionKind,
};
use jsonrpsee::http_client::HttpClient;
use test_cluster::TestCluster;

use crate::common::{ApiTestSetup, indexer_wait_for_checkpoint, indexer_wait_for_object};

type TxBytes = Base64;
type Signatures = Vec<Base64>;

async fn prepare_and_sign_tx(
    sender: IotaAddress,
    receiver: IotaAddress,
    cluster: &TestCluster,
    client: &HttpClient,
    obj_id: ObjectID,
    gas: ObjectID,
) -> (TxBytes, Signatures) {
    let transaction_bytes = client
        .transfer_object(sender, obj_id, Some(gas), 10_000_000.into(), receiver)
        .await
        .unwrap();

    let (tx_bytes, signatures) = cluster
        .wallet
        .sign_transaction(&transaction_bytes.to_data().unwrap())
        .to_tx_bytes_and_signatures();

    (tx_bytes, signatures)
}

async fn get_objects_to_mutate(
    cluster: &TestCluster,
    address: IotaAddress,
) -> (Vec<ObjectID>, ObjectID) {
    let owned_objects = cluster.get_owned_objects(address, None).await.unwrap();

    let gas = owned_objects.last().unwrap().object_id().unwrap();

    let object_ids = owned_objects
        .iter()
        .take(owned_objects.len() - 1)
        .map(|obj| obj.object_id().unwrap())
        .collect();

    (object_ids, gas)
}

#[ignore = "https://github.com/iotaledger/iota/issues/6120"]
#[test]
fn dry_run_transaction_block() {
    let ApiTestSetup {
        runtime,
        cluster,
        store,
        client,
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async {
        indexer_wait_for_checkpoint(store, 1).await;

        let sender = cluster.get_address_0();
        let receiver = cluster.get_address_1();

        let (objects, gas) = get_objects_to_mutate(cluster, sender).await;

        let (tx_bytes, signatures) =
            prepare_and_sign_tx(sender, receiver, cluster, client, objects[0], gas).await;

        let dry_run_tx_block_resp = client
            .dry_run_transaction_block(tx_bytes.clone())
            .await
            .unwrap();

        let indexer_tx_response = client
            .execute_transaction_block(
                tx_bytes,
                signatures,
                Some(
                    IotaTransactionBlockResponseOptions::new()
                        .with_effects()
                        .with_object_changes(),
                ),
                Some(ExecuteTransactionRequestType::WaitForLocalExecution),
            )
            .await
            .unwrap();

        assert_eq!(
            *indexer_tx_response.effects.as_ref().unwrap().status(),
            IotaExecutionStatus::Success
        );

        assert_eq!(
            indexer_tx_response.object_changes.unwrap(),
            dry_run_tx_block_resp.object_changes
        )
    });
}

#[test]
fn dev_inspect_transaction_block() {
    let ApiTestSetup {
        runtime,
        cluster,
        store,
        client,
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async {
        indexer_wait_for_checkpoint(store, 1).await;

        let (sender, _): (_, AccountKeyPair) = get_key_pair();
        let (receiver, _): (_, AccountKeyPair) = get_key_pair();

        let gas = cluster
            .fund_address_and_return_gas(
                cluster.get_reference_gas_price().await,
                Some(10_000_000_000),
                sender,
            )
            .await;

        indexer_wait_for_object(client, gas.0, gas.1).await;

        let (obj_id, seq_num, digest) = cluster
            .fund_address_and_return_gas(
                cluster.get_reference_gas_price().await,
                Some(10_000_000_000),
                sender,
            )
            .await;

        indexer_wait_for_object(client, obj_id, seq_num).await;

        let mut builder = ProgrammableTransactionBuilder::new();
        builder
            .transfer_object(receiver, (obj_id, seq_num, digest))
            .unwrap();
        let ptb = builder.finish();

        let indexer_devinspect_results = client
            .dev_inspect_transaction_block(
                sender,
                Base64::from_bytes(&bcs::to_bytes(&TransactionKind::programmable(ptb)).unwrap()),
                None,
                None,
                None,
            )
            .await
            .unwrap();

        assert_eq!(
            *indexer_devinspect_results.effects.status(),
            IotaExecutionStatus::Success
        );

        let owner = indexer_devinspect_results
            .effects
            .mutated()
            .iter()
            .find_map(|obj| (obj.reference.object_id == obj_id).then_some(obj.owner))
            .unwrap();

        assert_eq!(owner, Owner::AddressOwner(receiver));

        let latest_checkpoint_seq_number = client
            .get_latest_checkpoint_sequence_number()
            .await
            .unwrap();

        // Ensure that the actual object sequence number remains unchanged after the
        // checkpoint advances
        indexer_wait_for_checkpoint(store, latest_checkpoint_seq_number.into_inner() + 1).await;

        let actual_object_data = client
            .get_object(obj_id, Some(IotaObjectDataOptions::new().with_owner()))
            .await
            .unwrap()
            .data
            .unwrap();

        assert_eq!(
            actual_object_data.version, seq_num,
            "The object sequence number should not mutate"
        );
        assert_eq!(
            actual_object_data.owner.unwrap(),
            Owner::AddressOwner(sender),
            "The initial owner of the object should not change"
        );
    });
}

#[test]
fn execute_transaction_block() {
    let ApiTestSetup {
        runtime,
        cluster,
        store,
        client,
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async {
        indexer_wait_for_checkpoint(store, 1).await;

        let addresses = cluster.get_addresses();
        let sender = addresses[2];
        let receiver = addresses[3];

        let (objects, gas) = get_objects_to_mutate(cluster, sender).await;

        let obj_id = objects[0];

        let (tx_bytes, signatures) =
            prepare_and_sign_tx(sender, receiver, cluster, client, obj_id, gas).await;

        let indexer_tx_response = client
            .execute_transaction_block(
                tx_bytes,
                signatures,
                Some(IotaTransactionBlockResponseOptions::new().with_effects()),
                Some(ExecuteTransactionRequestType::WaitForLocalExecution),
            )
            .await
            .unwrap();

        assert_eq!(
            *indexer_tx_response.effects.as_ref().unwrap().status(),
            IotaExecutionStatus::Success
        );

        let (seq_num, owner) = indexer_tx_response
            .effects
            .unwrap()
            .mutated()
            .iter()
            .find_map(|obj| {
                (obj.reference.object_id == obj_id).then_some((obj.reference.version, obj.owner))
            })
            .unwrap();

        assert_eq!(owner, Owner::AddressOwner(receiver));

        indexer_wait_for_object(client, obj_id, seq_num).await;

        let actual_object_info = client
            .get_object(obj_id, Some(IotaObjectDataOptions::new().with_owner()))
            .await
            .unwrap();

        assert_eq!(
            actual_object_info.data.unwrap().owner.unwrap(),
            Owner::AddressOwner(receiver)
        );
    });
}
