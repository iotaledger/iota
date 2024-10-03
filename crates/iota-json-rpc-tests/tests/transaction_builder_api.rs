// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[cfg(not(msim))]
use std::str::FromStr;
use std::{collections::BTreeMap, path::Path};

use iota_json::{call_args, type_args};
use iota_json_rpc_api::{
    CoinReadApiClient, IndexerApiClient, TransactionBuilderClient, WriteApiClient,
};
use iota_json_rpc_types::{
    IotaObjectDataOptions, IotaObjectResponseQuery, IotaTransactionBlockEffectsAPI,
    IotaTransactionBlockResponse, IotaTransactionBlockResponseOptions, ObjectChange,
    TransactionBlockBytes,
};
use iota_macros::sim_test;
use iota_move_build::BuildConfig;
use iota_types::{
    base_types::{ObjectID, SequenceNumber},
    digests::ObjectDigest,
    gas_coin::GAS,
    object::Owner,
    quorum_driver_types::ExecuteTransactionRequestType,
    IOTA_FRAMEWORK_ADDRESS,
};
use test_cluster::TestClusterBuilder;

fn assert_same_object_changes_ignoring_version_and_digest(
    expected: Vec<ObjectChange>,
    actual: Vec<ObjectChange>,
) {
    fn collect_changes_mask_version_and_digest(
        changes: Vec<ObjectChange>,
    ) -> BTreeMap<ObjectID, ObjectChange> {
        changes
            .into_iter()
            .map(|mut change| {
                let object_id = change.object_id();
                // ignore the version and digest for comparison
                change.mask_for_test(SequenceNumber::MAX, ObjectDigest::MAX);
                (object_id, change)
            })
            .collect()
    }
    let expected = collect_changes_mask_version_and_digest(expected);
    let actual = collect_changes_mask_version_and_digest(actual);
    assert!(expected.keys().all(|id| actual.contains_key(id)));
    assert!(actual.keys().all(|id| expected.contains_key(id)));
    for (id, exp) in &expected {
        let act = actual.get(id).unwrap();
        assert_eq!(act, exp);
    }
}

#[sim_test]
async fn test_public_transfer_object() -> Result<(), anyhow::Error> {
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

    let obj = objects.clone().first().unwrap().object().unwrap().object_id;
    let gas = objects.clone().last().unwrap().object().unwrap().object_id;

    let transaction_bytes: TransactionBlockBytes = http_client
        .transfer_object(address, obj, Some(gas), 1_000_000.into(), address)
        .await?;

    let tx = cluster
        .wallet
        .sign_transaction(&transaction_bytes.to_data()?);
    let (tx_bytes, signatures) = tx.to_tx_bytes_and_signatures();
    let tx_bytes1 = tx_bytes.clone();
    let dryrun_response = http_client.dry_run_transaction_block(tx_bytes).await?;

    let tx_response: IotaTransactionBlockResponse = http_client
        .execute_transaction_block(
            tx_bytes1,
            signatures,
            Some(
                IotaTransactionBlockResponseOptions::new()
                    .with_effects()
                    .with_object_changes(),
            ),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await?;

    assert_same_object_changes_ignoring_version_and_digest(
        dryrun_response.object_changes,
        tx_response.object_changes.unwrap(),
    );
    Ok(())
}

#[sim_test]
async fn test_transfer_iota() -> Result<(), anyhow::Error> {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();
    let address = cluster.get_address_0();
    let other_address = cluster.get_address_1();

    let (gas, _, _) = cluster
        .wallet
        .get_one_gas_object_owned_by_address(address)
        .await?
        .unwrap();

    let transaction_bytes: TransactionBlockBytes = http_client
        .transfer_iota(
            address,
            gas,
            10_000_000.into(),
            other_address,
            Some(1234.into()),
        )
        .await?;

    let tx = cluster
        .wallet
        .sign_transaction(&transaction_bytes.to_data()?);
    let (tx_bytes, signatures) = tx.to_tx_bytes_and_signatures();

    let tx_response: IotaTransactionBlockResponse = http_client
        .execute_transaction_block(
            tx_bytes,
            signatures,
            Some(
                IotaTransactionBlockResponseOptions::new()
                    .with_effects()
                    .with_balance_changes(),
            ),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await?;
    assert_eq!(tx_response.status_ok(), Some(true));

    let gas_usage: i128 = tx_response
        .effects
        .unwrap()
        .gas_cost_summary()
        .net_gas_usage()
        .into();

    let balance_changes = tx_response.balance_changes.unwrap();
    assert_eq!(balance_changes[0].owner, Owner::AddressOwner(address));
    assert_eq!(balance_changes[0].amount, -1234 - gas_usage);
    assert_eq!(balance_changes[1].owner, Owner::AddressOwner(other_address));
    assert_eq!(balance_changes[1].amount, 1234);

    Ok(())
}

#[sim_test]
async fn test_pay() -> Result<(), anyhow::Error> {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();
    let address = cluster.get_address_0();
    let other_address = cluster.get_address_1();

    let gas_objs = cluster
        .wallet
        .get_gas_objects_owned_by_address(address, Some(2))
        .await?;
    let (gas_to_send, _, _) = gas_objs[0];
    let (gas_to_pay_for_tx, _, _) = gas_objs[1];

    let transaction_bytes: TransactionBlockBytes = http_client
        .pay(
            address,
            vec![gas_to_send],
            vec![other_address],
            vec![123.into()],
            Some(gas_to_pay_for_tx),
            10_000_000.into(),
        )
        .await?;

    let tx = cluster
        .wallet
        .sign_transaction(&transaction_bytes.to_data()?);
    let (tx_bytes, signatures) = tx.to_tx_bytes_and_signatures();

    let tx_response: IotaTransactionBlockResponse = http_client
        .execute_transaction_block(
            tx_bytes,
            signatures,
            Some(
                IotaTransactionBlockResponseOptions::new()
                    .with_effects()
                    .with_balance_changes(),
            ),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await?;
    assert_eq!(tx_response.status_ok(), Some(true));

    let gas_usage: i128 = tx_response
        .effects
        .unwrap()
        .gas_cost_summary()
        .net_gas_usage()
        .into();

    let balance_changes = tx_response.balance_changes.unwrap();
    assert_eq!(balance_changes[0].owner, Owner::AddressOwner(address));
    assert_eq!(balance_changes[0].amount, -123 - gas_usage);
    assert_eq!(balance_changes[1].owner, Owner::AddressOwner(other_address));
    assert_eq!(balance_changes[1].amount, 123);

    Ok(())
}

#[sim_test]
async fn test_pay_iota() -> Result<(), anyhow::Error> {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();
    let address = cluster.get_address_0();
    let recipient_1 = cluster.get_address_1();
    let recipient_2 = cluster.get_address_2();

    let coins = http_client
        .get_coins(address, None, None, Some(3))
        .await
        .unwrap()
        .data;

    let total_balance = coins.iter().map(|coin| coin.balance).sum::<u64>();
    let budget = 10_000_000;
    let amount_to_keep = 1000;
    let recipient_1_amount = 100_000_000;
    let recipient_2_amount = total_balance - budget - recipient_1_amount - amount_to_keep;

    let transaction_bytes: TransactionBlockBytes = http_client
        .pay_iota(
            address,
            coins.iter().map(|coin| coin.object_ref().0).collect(),
            vec![recipient_1, recipient_2],
            vec![recipient_1_amount.into(), recipient_2_amount.into()],
            budget.into(),
        )
        .await?;

    let tx = cluster
        .wallet
        .sign_transaction(&transaction_bytes.to_data()?);
    let (tx_bytes, signatures) = tx.to_tx_bytes_and_signatures();

    let tx_response: IotaTransactionBlockResponse = http_client
        .execute_transaction_block(
            tx_bytes,
            signatures,
            Some(
                IotaTransactionBlockResponseOptions::new()
                    .with_effects()
                    .with_balance_changes(),
            ),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await?;
    assert_eq!(tx_response.status_ok(), Some(true));

    let gas_usage: i128 = tx_response
        .effects
        .unwrap()
        .gas_cost_summary()
        .net_gas_usage()
        .into();

    let recipient_1_amount = i128::from(recipient_1_amount);
    let recipient_2_amount = i128::from(recipient_2_amount);

    let balance_changes = tx_response.balance_changes.unwrap();
    assert_eq!(balance_changes[0].owner, Owner::AddressOwner(address));
    assert_eq!(
        balance_changes[0].amount,
        -recipient_1_amount - recipient_2_amount - gas_usage
    );
    assert_eq!(balance_changes[1].owner, Owner::AddressOwner(recipient_1));
    assert_eq!(balance_changes[1].amount, recipient_1_amount);
    assert_eq!(balance_changes[2].owner, Owner::AddressOwner(recipient_2));
    assert_eq!(balance_changes[2].amount, recipient_2_amount);

    Ok(())
}

#[sim_test]
async fn test_pay_all_iota() -> Result<(), anyhow::Error> {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();
    let address = cluster.get_address_0();
    let recipient = cluster.get_address_1();

    let coins = http_client
        .get_coins(address, None, None, Some(3))
        .await
        .unwrap()
        .data;

    let total_balance = coins
        .iter()
        .map(|coin| i128::from(coin.balance))
        .sum::<i128>();
    let budget = 10_000_000;

    let transaction_bytes: TransactionBlockBytes = http_client
        .pay_all_iota(
            address,
            coins.iter().map(|coin| coin.object_ref().0).collect(),
            recipient,
            budget.into(),
        )
        .await?;

    let tx = cluster
        .wallet
        .sign_transaction(&transaction_bytes.to_data()?);
    let (tx_bytes, signatures) = tx.to_tx_bytes_and_signatures();

    let tx_response: IotaTransactionBlockResponse = http_client
        .execute_transaction_block(
            tx_bytes,
            signatures,
            Some(
                IotaTransactionBlockResponseOptions::new()
                    .with_effects()
                    .with_balance_changes(),
            ),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await?;
    assert_eq!(tx_response.status_ok(), Some(true));

    let gas_usage: i128 = tx_response
        .effects
        .unwrap()
        .gas_cost_summary()
        .net_gas_usage()
        .into();

    let balance_changes = tx_response.balance_changes.unwrap();
    assert_eq!(balance_changes[0].owner, Owner::AddressOwner(address));
    assert_eq!(balance_changes[0].amount, -total_balance);
    assert_eq!(balance_changes[1].owner, Owner::AddressOwner(recipient));
    assert_eq!(balance_changes[1].amount, total_balance - gas_usage);

    Ok(())
}

#[sim_test]
async fn test_publish() -> Result<(), anyhow::Error> {
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
        .await?;
    let gas = objects.data.first().unwrap().object().unwrap();

    let compiled_package =
        BuildConfig::new_for_testing().build(Path::new("../../examples/move/basics"))?;
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
        .await?;

    let tx = cluster
        .wallet
        .sign_transaction(&transaction_bytes.to_data()?);
    let (tx_bytes, signatures) = tx.to_tx_bytes_and_signatures();

    let tx_response = http_client
        .execute_transaction_block(
            tx_bytes,
            signatures,
            Some(IotaTransactionBlockResponseOptions::new().with_effects()),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await?;
    matches!(tx_response, IotaTransactionBlockResponse {effects, ..} if effects.as_ref().unwrap().created().len() == 6);
    Ok(())
}

#[sim_test]
async fn test_move_call() -> Result<(), anyhow::Error> {
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

    let gas = objects.first().unwrap().object().unwrap();
    let coin = &objects[1].object()?;

    // now do the call
    let package_id = ObjectID::new(IOTA_FRAMEWORK_ADDRESS.into_bytes());
    let module = "pay".to_string();
    let function = "split".to_string();

    let transaction_bytes: TransactionBlockBytes = http_client
        .move_call(
            address,
            package_id,
            module,
            function,
            type_args![GAS::type_tag()]?,
            call_args!(coin.object_id, 10)?,
            Some(gas.object_id),
            10_000_000.into(),
            None,
        )
        .await?;

    let tx = cluster
        .wallet
        .sign_transaction(&transaction_bytes.to_data()?);

    let (tx_bytes, signatures) = tx.to_tx_bytes_and_signatures();

    let tx_response = http_client
        .execute_transaction_block(
            tx_bytes,
            signatures,
            Some(IotaTransactionBlockResponseOptions::new().with_effects()),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await?;
    matches!(tx_response, IotaTransactionBlockResponse {effects, ..} if effects.as_ref().unwrap().created().len() == 1);
    Ok(())
}
