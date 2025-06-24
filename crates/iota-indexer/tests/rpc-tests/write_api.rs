// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
use std::{path::Path, str::FromStr};

use fastcrypto::encoding::Base64;
use iota_json::{call_args, type_args};
use iota_json_rpc_api::{ReadApiClient, TransactionBuilderClient, WriteApiClient};
use iota_json_rpc_types::{
    IotaExecutionStatus, IotaObjectDataOptions, IotaTransactionBlockEffectsAPI,
    IotaTransactionBlockResponse, IotaTransactionBlockResponseOptions, ObjectChange,
    TransactionBlockBytes,
};
use iota_move_build::BuildConfig;
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::{
    base_types::{IotaAddress, ObjectID, ObjectRef},
    crypto::{AccountKeyPair, get_key_pair},
    gas_coin::NANOS_PER_IOTA,
    object::Owner,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    quorum_driver_types::ExecuteTransactionRequestType,
    transaction::TransactionKind,
    utils::to_sender_signed_transaction,
};
use itertools::Itertools;
use jsonrpsee::http_client::HttpClient;

use crate::common::{ApiTestSetup, indexer_wait_for_checkpoint, indexer_wait_for_object};
type TxBytes = Base64;
type Signatures = Vec<Base64>;

async fn prepare_and_sign_object_transfer_tx(
    sender: IotaAddress,
    sender_key_pair: AccountKeyPair,
    receiver: IotaAddress,
    object_to_transfer: ObjectRef,
    gas: ObjectRef,
) -> (TxBytes, Signatures) {
    let tx_builder = TestTransactionBuilder::new(sender, gas, 1000);
    let tx_data = tx_builder.transfer(object_to_transfer, receiver).build();
    let signed_transaction = to_sender_signed_transaction(tx_data, &sender_key_pair);
    signed_transaction.to_tx_bytes_and_signatures()
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

        let (sender, key_pair): (_, AccountKeyPair) = get_key_pair();
        let (receiver, _): (_, AccountKeyPair) = get_key_pair();

        let gas = cluster
            .fund_address_and_return_gas(
                cluster.get_reference_gas_price().await,
                Some(NANOS_PER_IOTA),
                sender,
            )
            .await;
        indexer_wait_for_object(client, gas.0, gas.1).await;

        let object_to_transfer = cluster
            .fund_address_and_return_gas(
                cluster.get_reference_gas_price().await,
                Some(NANOS_PER_IOTA),
                sender,
            )
            .await;
        indexer_wait_for_object(client, object_to_transfer.0, object_to_transfer.1).await;

        let (tx_bytes, signatures) = prepare_and_sign_object_transfer_tx(
            sender,
            key_pair,
            receiver,
            object_to_transfer,
            gas,
        )
        .await;

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
        );

        assert!(
            dry_run_tx_block_resp
                .effects
                .mutated()
                .iter()
                .any(|obj| obj.reference.object_id == object_to_transfer.0)
        );
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

        let (sender, key_pair): (_, AccountKeyPair) = get_key_pair();
        let (receiver, _): (_, AccountKeyPair) = get_key_pair();

        let gas = cluster
            .fund_address_and_return_gas(
                cluster.get_reference_gas_price().await,
                Some(NANOS_PER_IOTA),
                sender,
            )
            .await;
        indexer_wait_for_object(client, gas.0, gas.1).await;

        let object_to_transfer = cluster
            .fund_address_and_return_gas(
                cluster.get_reference_gas_price().await,
                Some(NANOS_PER_IOTA),
                sender,
            )
            .await;
        indexer_wait_for_object(client, object_to_transfer.0, object_to_transfer.1).await;

        let object_to_transfer_id = object_to_transfer.0;

        let (tx_bytes, signatures) = prepare_and_sign_object_transfer_tx(
            sender,
            key_pair,
            receiver,
            object_to_transfer,
            gas,
        )
        .await;

        let indexer_tx_response = client
            .execute_transaction_block(
                tx_bytes,
                signatures,
                Some(IotaTransactionBlockResponseOptions::new().with_effects()),
                Some(ExecuteTransactionRequestType::WaitForLocalExecution),
            )
            .await
            .unwrap();
        assert_eq!(indexer_tx_response.status_ok(), Some(true));

        let (seq_num, owner) = indexer_tx_response
            .effects
            .unwrap()
            .mutated()
            .iter()
            .find_map(|obj| {
                (obj.reference.object_id == object_to_transfer_id)
                    .then_some((obj.reference.version, obj.owner))
            })
            .unwrap();

        assert_eq!(owner, Owner::AddressOwner(receiver));

        let actual_object_info = client
            .get_object(
                object_to_transfer_id,
                Some(IotaObjectDataOptions::new().with_owner()),
            )
            .await
            .unwrap();

        assert_eq!(actual_object_info.data.as_ref().unwrap().version, seq_num);
        assert_eq!(
            actual_object_info.data.unwrap().owner.unwrap(),
            Owner::AddressOwner(receiver)
        );
    });
}

#[test]
fn test_execute_transactions_with_shared_objects() {
    let ApiTestSetup {
        runtime,
        cluster,
        store,
        client,
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async {
        indexer_wait_for_checkpoint(store, 1).await;

        let (sender, sender_kp): (_, AccountKeyPair) = get_key_pair();

        let gas = cluster
            .fund_address_and_return_gas(
                cluster.get_reference_gas_price().await,
                Some(10_000_000_000),
                sender,
            )
            .await;

        indexer_wait_for_object(client, gas.0, gas.1).await;

        let res = deploy_basics_pkg(sender, &sender_kp, client).await;

        let package_id = res
            .object_changes
            .as_ref()
            .unwrap()
            .iter()
            .filter_map(|o| match o {
                ObjectChange::Published { package_id, .. } => Some(package_id),
                _ => None,
            })
            .exactly_one()
            .unwrap();

        let (_, counter_obj) = create_counter_object(sender, &sender_kp, client, package_id)
            .await
            .unwrap();

        let res_1 = increment_counter(sender, &sender_kp, client, package_id, &counter_obj)
            .await
            .unwrap();
        assert_eq!(res_1.status_ok(), Some(true));

        // TODO: extend with subsequent call to the same object once race
        // conditions are fixed
    });
}

async fn create_counter_object(
    address: IotaAddress,
    address_kp: &AccountKeyPair,
    client: &HttpClient,
    package_id: &ObjectID,
) -> Result<(IotaTransactionBlockResponse, ObjectID), anyhow::Error> {
    let module = "counter".to_string();
    let tx_bytes: TransactionBlockBytes = client
        .move_call(
            address,
            *package_id,
            module.clone(),
            "create".to_string(),
            type_args![].unwrap(),
            call_args!().unwrap(),
            None,
            10_000_000.into(),
            None,
        )
        .await?;
    let txn = to_sender_signed_transaction(tx_bytes.to_data().unwrap(), address_kp);
    let (tx_bytes, signatures) = txn.to_tx_bytes_and_signatures();

    let res = client
        .execute_transaction_block(
            tx_bytes,
            signatures,
            Some(IotaTransactionBlockResponseOptions::full_content()),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await
        .unwrap();
    let counter_obj_id = res
        .effects
        .as_ref()
        .unwrap()
        .created()
        .iter()
        .exactly_one()
        .unwrap()
        .object_id();
    Ok((res, counter_obj_id))
}

async fn increment_counter(
    address: IotaAddress,
    address_kp: &AccountKeyPair,
    client: &HttpClient,
    package_id: &ObjectID,
    counter_id: &ObjectID,
) -> Result<IotaTransactionBlockResponse, anyhow::Error> {
    let module = "counter".to_string();
    let function = "increment".to_string();
    let tx_bytes = client
        .move_call(
            address,
            *package_id,
            module.clone(),
            function.clone(),
            type_args![].unwrap(),
            call_args!(counter_id).unwrap(),
            None,
            10_000_000.into(),
            None,
        )
        .await
        .unwrap();
    let txn = to_sender_signed_transaction(tx_bytes.to_data().unwrap(), address_kp);
    let (tx_bytes, signatures) = txn.to_tx_bytes_and_signatures();

    let res = client
        .execute_transaction_block(
            tx_bytes,
            signatures,
            Some(IotaTransactionBlockResponseOptions::full_content()),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await
        .unwrap();
    Ok(res)
}

async fn deploy_basics_pkg(
    address: IotaAddress,
    address_kp: &AccountKeyPair,
    client: &HttpClient,
) -> IotaTransactionBlockResponse {
    deploy_package(address, address_kp, client, "../../examples/move/basics").await
}

async fn deploy_package(
    address: IotaAddress,
    address_kp: &AccountKeyPair,
    client: &HttpClient,
    pkg_path: &str,
) -> IotaTransactionBlockResponse {
    let compiled_package = BuildConfig::new_for_testing()
        .build(Path::new(pkg_path))
        .unwrap();
    let compiled_modules_bytes =
        compiled_package.get_package_base64(/* with_unpublished_deps */ false);
    let dependencies = compiled_package.get_dependency_storage_package_ids();

    let tx_bytes: TransactionBlockBytes = client
        .publish(
            address,
            compiled_modules_bytes,
            dependencies,
            None,
            100_000_000.into(),
        )
        .await
        .unwrap();

    let txn = to_sender_signed_transaction(tx_bytes.to_data().unwrap(), address_kp);

    let (tx_bytes, signatures) = txn.to_tx_bytes_and_signatures();
    client
        .execute_transaction_block(
            tx_bytes,
            signatures,
            Some(IotaTransactionBlockResponseOptions::full_content()),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await
        .unwrap()
}
