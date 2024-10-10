// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0


use iota_json_rpc_api::{IndexerApiClient, TransactionBuilderClient, WriteApiClient};
use iota_json_rpc_types::{
    IotaObjectDataOptions, IotaObjectResponseQuery, IotaTransactionBlockResponse,
    IotaTransactionBlockResponseOptions, TransactionBlockBytes,
};
use iota_macros::sim_test;
use iota_types::{
    quorum_driver_types::ExecuteTransactionRequestType,
    transaction::SenderSignedData,
};
use test_cluster::TestClusterBuilder;

#[sim_test]
async fn test_get_transaction_block() -> Result<(), anyhow::Error> {
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
    let gas_id = objects.last().unwrap().object().unwrap().object_id;

    // Make some transactions
    let mut tx_responses: Vec<IotaTransactionBlockResponse> = Vec::new();
    for obj in &objects[..objects.len() - 1] {
        let oref = obj.object().unwrap();
        let transaction_bytes: TransactionBlockBytes = http_client
            .transfer_object(
                address,
                oref.object_id,
                Some(gas_id),
                1_000_000.into(),
                address,
            )
            .await?;
        let tx = cluster
            .wallet
            .sign_transaction(&transaction_bytes.to_data()?);

        let (tx_bytes, signatures) = tx.to_tx_bytes_and_signatures();

        let response = http_client
            .execute_transaction_block(
                tx_bytes,
                signatures,
                Some(IotaTransactionBlockResponseOptions::new()),
                Some(ExecuteTransactionRequestType::WaitForLocalExecution),
            )
            .await?;

        tx_responses.push(response);
    }

    // TODO(chris): re-enable after rewriting get_transactions_in_range_deprecated
    // with query_transactions test get_transaction_batch
    // let batch_responses: Vec<IotaTransactionBlockResponse> = http_client
    //     .multi_get_transaction_blocks(tx,
    // Some(IotaTransactionBlockResponseOptions::new()))     .await?;

    // assert_eq!(5, batch_responses.len());

    // for r in batch_responses.iter().skip(1) {
    //     assert!(tx_responses
    //         .iter()
    //         .any(|resp| matches!(resp, IotaTransactionBlockResponse {digest, ..}
    // if *digest == r.digest))) }

    // // test get_transaction
    // for tx_digest in tx {
    //     let response: IotaTransactionBlockResponse = http_client
    //         .get_transaction_block(
    //             tx_digest,
    // Some(IotaTransactionBlockResponseOptions::new().with_raw_input()),
    //         )
    //         .await?;
    //     assert!(tx_responses.iter().any(
    //         |resp| matches!(resp, IotaTransactionBlockResponse {digest, ..} if *digest == response.digest)
    //     ));
    //     let sender_signed_data: SenderSignedData =
    //         bcs::from_bytes(&response.raw_transaction).unwrap();
    //     assert_eq!(sender_signed_data.digest(), tx_digest);
    // }

    Ok(())
}

#[sim_test]
async fn test_get_raw_transaction() -> Result<(), anyhow::Error> {
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
        .await?
        .data;
    let object_to_transfer = objects.first().unwrap().object().unwrap().object_id;

    // Make a transfer transactions
    let transaction_bytes: TransactionBlockBytes = http_client
        .transfer_object(address, object_to_transfer, None, 1_000_000.into(), address)
        .await?;
    let tx = cluster
        .wallet
        .sign_transaction(&transaction_bytes.to_data()?);
    let original_sender_signed_data = tx.data().clone();

    let (tx_bytes, signatures) = tx.to_tx_bytes_and_signatures();

    let response = http_client
        .execute_transaction_block(
            tx_bytes,
            signatures,
            Some(IotaTransactionBlockResponseOptions::new().with_raw_input()),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await?;

    let decode_sender_signed_data: SenderSignedData =
        bcs::from_bytes(&response.raw_transaction).unwrap();
    // verify that the raw transaction data returned by the response is the same
    // as the original transaction data
    assert_eq!(decode_sender_signed_data, original_sender_signed_data);

    Ok(())
}
