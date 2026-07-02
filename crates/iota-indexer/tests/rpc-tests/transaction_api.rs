// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_json_rpc_api::{ReadApiClient, TransactionBuilderClient, WriteApiClient};
use iota_json_rpc_types::{IotaTransactionBlockResponseOptions, TransactionBlockBytes};
use iota_sdk_types::Address;
use iota_types::{
    base_types::ObjectRef,
    crypto::{AccountKeyPair, get_key_pair},
    quorum_driver_types::ExecuteTransactionRequestType,
    transaction::SenderSignedData,
    utils::to_sender_signed_transaction,
};

use crate::common::{
    ApiTestSetup, execute_tx_and_wait_for_indexer_checkpoint, indexer_wait_for_checkpoint,
    indexer_wait_for_object,
};

/// Fund an address with a gas coin and an object to transfer, waiting for both
/// to be indexed.
async fn fund_gas_and_object(setup: &ApiTestSetup, sender: Address) -> (ObjectRef, ObjectRef) {
    let ApiTestSetup {
        cluster, client, ..
    } = setup;

    let rgp = cluster.get_reference_gas_price().await;
    let gas = cluster
        .fund_address_and_return_gas(rgp, Some(10_000_000_000), sender)
        .await;
    let object = cluster
        .fund_address_and_return_gas(rgp, Some(10_000_000_000), sender)
        .await;
    indexer_wait_for_object(client, gas.object_id, gas.version).await;
    indexer_wait_for_object(client, object.object_id, object.version).await;
    (gas, object)
}

#[test]
fn get_transaction_block() {
    let setup = ApiTestSetup::get_or_init();
    let ApiTestSetup {
        runtime,
        store,
        client,
        ..
    } = setup;

    runtime.block_on(async move {
        indexer_wait_for_checkpoint(store, 1).await;

        let (sender, keypair): (_, AccountKeyPair) = get_key_pair();
        let (receiver, _): (_, AccountKeyPair) = get_key_pair();

        let (gas, object) = fund_gas_and_object(setup, sender).await;

        let tx_bytes: TransactionBlockBytes = client
            .transfer_object(
                sender,
                object.object_id,
                Some(gas.object_id),
                10_000_000.into(),
                receiver,
            )
            .await
            .unwrap();
        let digest =
            execute_tx_and_wait_for_indexer_checkpoint(client, store, tx_bytes, &keypair).await;

        // The executed transaction can be fetched back from the indexer by digest.
        let tx = client
            .get_transaction_block(digest, Some(IotaTransactionBlockResponseOptions::new()))
            .await
            .unwrap();
        assert_eq!(tx.digest, digest);
    });
}

#[test]
fn get_raw_transaction() {
    let setup = ApiTestSetup::get_or_init();
    let ApiTestSetup {
        runtime,
        store,
        client,
        ..
    } = setup;

    runtime.block_on(async move {
        indexer_wait_for_checkpoint(store, 1).await;

        let (sender, keypair): (_, AccountKeyPair) = get_key_pair();
        let (receiver, _): (_, AccountKeyPair) = get_key_pair();

        let (gas, object) = fund_gas_and_object(setup, sender).await;

        let transaction_bytes: TransactionBlockBytes = client
            .transfer_object(
                sender,
                object.object_id,
                Some(gas.object_id),
                10_000_000.into(),
                receiver,
            )
            .await
            .unwrap();

        // `sender` is a freshly generated address (funded via the faucet), so it must
        // be signed with its own key, not the cluster wallet.
        let txn = to_sender_signed_transaction(transaction_bytes.to_data().unwrap(), &keypair);
        let original_sender_signed_data = txn.data().clone();

        let (tx_bytes, signatures) = txn.to_tx_bytes_and_signatures();

        let response = client
            .execute_transaction_block(
                tx_bytes,
                signatures,
                Some(IotaTransactionBlockResponseOptions::new().with_raw_input()),
                Some(ExecuteTransactionRequestType::WaitForLocalExecution.into()),
            )
            .await
            .unwrap();

        // The raw transaction bytes returned round-trip back to the original data.
        let decoded_sender_signed_data: SenderSignedData =
            bcs::from_bytes(&response.raw_transaction).unwrap();
        assert_eq!(decoded_sender_signed_data, original_sender_signed_data);
    });
}
