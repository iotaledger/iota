// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use fastcrypto::encoding::Base64;
use iota_json_rpc_api::{
    IndexerApiClient, ReadApiClient, TransactionBuilderClient, WriteApiClient,
};
use iota_json_rpc_types::{
    IotaExecutionStatus, IotaObjectDataOptions, IotaTransactionBlockEffectsAPI,
    IotaTransactionBlockResponseOptions,
};
use iota_types::{
    base_types::{IotaAddress, ObjectID},
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
async fn prepare_and_sign_to_transfer_first_object(
    sender: IotaAddress,
    receiver: IotaAddress,
    cluster: &TestCluster,
    client: &HttpClient,
) -> (ObjectID, TxBytes, Signatures) {
    let objects = cluster
        .rpc_client()
        .get_owned_objects(sender, None, None, None)
        .await
        .unwrap()
        .data;

    let obj_id = objects.first().unwrap().object().unwrap().object_id;
    let gas = objects.last().unwrap().object().unwrap().object_id;

    let transaction_bytes = client
        .transfer_object(sender, obj_id, Some(gas), 10_000_000.into(), receiver)
        .await
        .unwrap();

    let (tx_bytes, signatures) = cluster
        .wallet
        .sign_transaction(&transaction_bytes.to_data().unwrap())
        .to_tx_bytes_and_signatures();

    (obj_id, tx_bytes, signatures)
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

        let sender = cluster.get_address_0();
        let receiver = cluster.get_address_1();

        let objects = cluster
            .rpc_client()
            .get_owned_objects(sender, None, None, None)
            .await
            .unwrap()
            .data;

        let (obj_id, seq_num, digest) = objects.first().unwrap().object().unwrap().object_ref();

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

        assert_eq!(
            indexer_devinspect_results
                .effects
                .mutated()
                .iter()
                .find_map(|o| { (o.reference.object_id == obj_id).then_some(o.owner) })
                .unwrap(),
            Owner::AddressOwner(receiver)
        );

        indexer_wait_for_object(store, obj_id, seq_num).await;

        let actual_object_info = client
            .get_object(obj_id, Some(IotaObjectDataOptions::new().with_owner()))
            .await
            .unwrap();

        assert_eq!(
            actual_object_info.data.unwrap().owner.unwrap(),
            Owner::AddressOwner(sender)
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

        let sender = cluster.get_address_0();
        let receiver = cluster.get_address_1();

        let (obj_id, tx_bytes, signatures) =
            prepare_and_sign_to_transfer_first_object(sender, receiver, cluster, client).await;

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
            .find_map(|o| {
                (o.reference.object_id == obj_id).then_some((o.reference.version, o.owner))
            })
            .unwrap();

        assert_eq!(owner, Owner::AddressOwner(receiver));

        indexer_wait_for_object(store, obj_id, seq_num).await;

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

        let (_, tx_bytes, signatures) =
            prepare_and_sign_to_transfer_first_object(sender, receiver, cluster, client).await;

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
