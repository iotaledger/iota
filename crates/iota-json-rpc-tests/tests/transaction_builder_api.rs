// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[cfg(not(msim))]
use std::str::FromStr;
use std::{collections::BTreeMap, path::Path};

use iota_json::{call_args, type_args};
use iota_json_rpc_api::{
    CoinReadApiClient, IndexerApiClient, ReadApiClient, TransactionBuilderClient, WriteApiClient,
};
use iota_json_rpc_types::{
    IotaObjectDataOptions, IotaObjectResponseQuery, IotaTransactionBlockEffectsAPI,
    IotaTransactionBlockResponse, IotaTransactionBlockResponseOptions, MoveCallParams,
    ObjectChange, RPCTransactionRequestParams, TransactionBlockBytes, TransferObjectParams,
};
use iota_macros::sim_test;
use iota_move_build::BuildConfig;
use iota_types::{
    IOTA_FRAMEWORK_ADDRESS,
    base_types::{ObjectID, SequenceNumber},
    digests::ObjectDigest,
    gas_coin::GAS,
    object::Owner,
    quorum_driver_types::ExecuteTransactionRequestType,
};
use jsonrpsee::http_client::HttpClient;
use test_cluster::{TestCluster, TestClusterBuilder};

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
        .transfer_object(address, obj, Some(gas), 10_000_000.into(), address)
        .await?;

    let tx = cluster
        .wallet
        .sign_transaction(&transaction_bytes.clone().to_data()?);
    let (tx_bytes, _signatures) = tx.to_tx_bytes_and_signatures();

    let dryrun_response = http_client.dry_run_transaction_block(tx_bytes).await?;

    let tx_response = execute_tx(&cluster, http_client, transaction_bytes)
        .await
        .unwrap();

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

    let amount_to_transfer: i128 = 1234;
    let transaction_bytes: TransactionBlockBytes = http_client
        .transfer_iota(
            address,
            gas,
            10_000_000.into(),
            other_address,
            Some(u64::try_from(amount_to_transfer).unwrap().into()),
        )
        .await?;

    let tx_response = execute_tx(&cluster, http_client, transaction_bytes)
        .await
        .unwrap();

    let gas_usage: i128 = tx_response
        .effects
        .unwrap()
        .gas_cost_summary()
        .net_gas_usage()
        .into();

    // let amount_to_transfer = i128::from(amount_to_transfer);
    let expected_sender_balance_change = -amount_to_transfer - gas_usage;
    let balance_changes = tx_response.balance_changes.unwrap();
    assert_eq!(balance_changes[0].owner, Owner::AddressOwner(address));
    assert_eq!(balance_changes[0].amount, expected_sender_balance_change);
    assert_eq!(balance_changes[1].owner, Owner::AddressOwner(other_address));
    assert_eq!(balance_changes[1].amount, amount_to_transfer);

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

    let amount_to_transfer: i128 = 123;
    let transaction_bytes: TransactionBlockBytes = http_client
        .pay(
            address,
            vec![gas_to_send],
            vec![other_address],
            vec![u64::try_from(amount_to_transfer).unwrap().into()],
            Some(gas_to_pay_for_tx),
            10_000_000.into(),
        )
        .await?;

    let tx_response = execute_tx(&cluster, http_client, transaction_bytes)
        .await
        .unwrap();

    let gas_usage: i128 = tx_response
        .effects
        .unwrap()
        .gas_cost_summary()
        .net_gas_usage()
        .into();

    let expected_sender_balance_change = -amount_to_transfer - gas_usage;
    let balance_changes = tx_response.balance_changes.unwrap();
    assert_eq!(balance_changes[0].owner, Owner::AddressOwner(address));
    assert_eq!(balance_changes[0].amount, expected_sender_balance_change);
    assert_eq!(balance_changes[1].owner, Owner::AddressOwner(other_address));
    assert_eq!(balance_changes[1].amount, amount_to_transfer);

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

    let tx_response = execute_tx(&cluster, http_client, transaction_bytes)
        .await
        .unwrap();

    let gas_usage: i128 = tx_response
        .effects
        .unwrap()
        .gas_cost_summary()
        .net_gas_usage()
        .into();

    let recipient_1_amount = i128::from(recipient_1_amount);
    let recipient_2_amount = i128::from(recipient_2_amount);

    let expected_sender_balance_change = -recipient_1_amount - recipient_2_amount - gas_usage;
    let balance_changes = tx_response.balance_changes.unwrap();
    assert_eq!(balance_changes[0].owner, Owner::AddressOwner(address));
    assert_eq!(balance_changes[0].amount, expected_sender_balance_change);
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

    let tx_response = execute_tx(&cluster, http_client, transaction_bytes)
        .await
        .unwrap();

    let gas_usage: i128 = tx_response
        .effects
        .unwrap()
        .gas_cost_summary()
        .net_gas_usage()
        .into();

    let expected_recipient_balance_change = total_balance - gas_usage;
    let balance_changes = tx_response.balance_changes.unwrap();
    assert_eq!(balance_changes[0].owner, Owner::AddressOwner(address));
    assert_eq!(balance_changes[0].amount, -total_balance);
    assert_eq!(balance_changes[1].owner, Owner::AddressOwner(recipient));
    assert_eq!(balance_changes[1].amount, expected_recipient_balance_change);

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

    let tx_response = execute_tx(&cluster, http_client, transaction_bytes)
        .await
        .unwrap();

    matches!(tx_response, IotaTransactionBlockResponse {effects, ..} if effects.as_ref().unwrap().created().len() == 6);
    Ok(())
}

#[sim_test]
async fn test_split_coin() -> Result<(), anyhow::Error> {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();
    let address = cluster.get_address_0();

    let coins = http_client
        .get_coins(address, None, None, Some(2))
        .await
        .unwrap()
        .data;
    let coin_to_split = &coins[0];
    let gas = &coins[1];

    let new_coin_1_amount = 100;
    let new_coin_2_amount = 200;

    let transaction_bytes: TransactionBlockBytes = http_client
        .split_coin(
            address,
            coin_to_split.coin_object_id,
            vec![new_coin_1_amount.into(), new_coin_2_amount.into()],
            Some(gas.coin_object_id),
            10_000_000.into(),
        )
        .await?;

    let tx_response = execute_tx(&cluster, http_client, transaction_bytes)
        .await
        .unwrap();

    let new_coin_ids: Vec<ObjectID> = tx_response
        .effects
        .unwrap()
        .created()
        .iter()
        .map(|coin| coin.object_id())
        .collect();
    let coins = http_client
        .get_coins(address, None, None, Some(100))
        .await
        .unwrap()
        .data;

    let mut new_coin_balances: Vec<u64> = coins
        .iter()
        .filter(|coin| new_coin_ids.contains(&coin.coin_object_id))
        .map(|coin| coin.balance)
        .collect();
    new_coin_balances.sort();
    let split_coin_balance = coins
        .iter()
        .find(|coin| coin.coin_object_id == coin_to_split.coin_object_id)
        .unwrap()
        .balance;

    let expected_split_coin_balance = coin_to_split.balance - new_coin_1_amount - new_coin_2_amount;

    assert_eq!(
        new_coin_balances,
        vec![new_coin_1_amount, new_coin_2_amount]
    );
    assert_eq!(split_coin_balance, expected_split_coin_balance);

    Ok(())
}

#[sim_test]
async fn test_split_coin_equal() -> Result<(), anyhow::Error> {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();
    let address = cluster.get_address_0();

    let coins = http_client
        .get_coins(address, None, None, Some(2))
        .await
        .unwrap()
        .data;
    let coin_to_split = &coins[0];
    let gas = &coins[1];

    let split_count = 7;

    let transaction_bytes: TransactionBlockBytes = http_client
        .split_coin_equal(
            address,
            coin_to_split.coin_object_id,
            split_count.into(),
            Some(gas.coin_object_id),
            10_000_000.into(),
        )
        .await?;

    let tx_response = execute_tx(&cluster, http_client, transaction_bytes)
        .await
        .unwrap();

    let new_coin_ids: Vec<ObjectID> = tx_response
        .effects
        .unwrap()
        .created()
        .iter()
        .map(|coin| coin.object_id())
        .collect();
    let coins = http_client
        .get_coins(address, None, None, Some(100))
        .await
        .unwrap()
        .data;
    let mut new_coin_balances: Vec<u64> = coins
        .iter()
        .filter(|coin| new_coin_ids.contains(&coin.coin_object_id))
        .map(|coin| coin.balance)
        .collect();
    new_coin_balances.sort();
    let split_coin_balance = coins
        .iter()
        .find(|coin| coin.coin_object_id == coin_to_split.coin_object_id)
        .unwrap()
        .balance;

    let original_coin_balance = coin_to_split.balance;
    let expected_new_coin_balances = 4_285_714_285_714_285;
    let expected_split_coin_balance =
        original_coin_balance - (expected_new_coin_balances * (split_count - 1));

    assert_eq!(original_coin_balance, 30_000_000_000_000_000);
    assert_eq!(expected_split_coin_balance, 4_285_714_285_714_290);
    assert_eq!(
        new_coin_balances,
        vec![
            expected_new_coin_balances,
            expected_new_coin_balances,
            expected_new_coin_balances,
            expected_new_coin_balances,
            expected_new_coin_balances,
            expected_new_coin_balances
        ]
    );
    assert_eq!(split_coin_balance, expected_split_coin_balance);

    Ok(())
}

#[sim_test]
async fn test_merge_coin() -> Result<(), anyhow::Error> {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();
    let address = cluster.get_address_0();

    let coins = http_client
        .get_coins(address, None, None, Some(3))
        .await
        .unwrap()
        .data;
    let primary_coin = &coins[0];
    let coin_to_merge = &coins[1];
    let gas = &coins[2];

    let transaction_bytes: TransactionBlockBytes = http_client
        .merge_coin(
            address,
            primary_coin.coin_object_id,
            coin_to_merge.coin_object_id,
            Some(gas.coin_object_id),
            10_000_000.into(),
        )
        .await?;

    let tx_response = execute_tx(&cluster, http_client, transaction_bytes)
        .await
        .unwrap();

    let deleted_coin_ids: Vec<ObjectID> = tx_response
        .effects
        .unwrap()
        .deleted()
        .iter()
        .map(|coin| coin.object_id)
        .collect();
    let coins = http_client
        .get_coins(address, None, None, Some(100))
        .await
        .unwrap()
        .data;
    let new_primary_coin_balance = coins
        .iter()
        .find(|coin| coin.coin_object_id == primary_coin.coin_object_id)
        .unwrap()
        .balance;
    let expected_balance_after_merge = primary_coin.balance + coin_to_merge.balance;

    assert_eq!(new_primary_coin_balance, expected_balance_after_merge);
    assert_eq!(deleted_coin_ids, vec![coin_to_merge.coin_object_id]);

    Ok(())
}

#[sim_test]
async fn test_batch_transaction() -> Result<(), anyhow::Error> {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();
    let address = cluster.get_address_0();
    let other_address = cluster.get_address_1();

    let coins = http_client
        .get_coins(address, None, None, Some(3))
        .await
        .unwrap()
        .data;
    let coin_to_split = &coins[0];
    let coin_to_transfer = &coins[1];
    let gas = &coins[2];
    let amount_to_split = 10;

    let transaction_bytes: TransactionBlockBytes = http_client
        .batch_transaction(
            address,
            vec![
                RPCTransactionRequestParams::MoveCallRequestParams(MoveCallParams {
                    package_object_id: ObjectID::new(IOTA_FRAMEWORK_ADDRESS.into_bytes()),
                    module: "pay".to_string(),
                    function: "split".to_string(),
                    type_arguments: type_args![GAS::type_tag()]?,
                    arguments: call_args!(coin_to_split.coin_object_id, amount_to_split)?,
                }),
                RPCTransactionRequestParams::TransferObjectRequestParams(TransferObjectParams {
                    recipient: other_address,
                    object_id: coin_to_transfer.coin_object_id,
                }),
            ],
            Some(gas.coin_object_id),
            10_000_000.into(),
            None,
        )
        .await?;

    let tx_response = execute_tx(&cluster, http_client, transaction_bytes)
        .await
        .unwrap();

    // Assert results of the move call
    {
        let created_coin_ids: Vec<ObjectID> = tx_response
            .effects
            .unwrap()
            .created()
            .iter()
            .map(|coin| coin.object_id())
            .collect();
        let coins = http_client
            .get_coins(address, None, None, Some(100))
            .await
            .unwrap()
            .data;
        let split_coin_balance = coins
            .iter()
            .find(|coin| coin.coin_object_id == coin_to_split.coin_object_id)
            .unwrap()
            .balance;
        let new_coin_balances: Vec<u64> = coins
            .iter()
            .filter(|coin| created_coin_ids.contains(&coin.coin_object_id))
            .map(|coin| coin.balance)
            .collect();
        let expected_split_coin_balance = coin_to_split.balance - amount_to_split;

        assert_eq!(split_coin_balance, expected_split_coin_balance);
        assert_eq!(new_coin_balances, vec![amount_to_split]);
    }

    // Assert results of object transfer
    {
        let transferred_object = http_client
            .get_object(
                coin_to_transfer.coin_object_id,
                Some(IotaObjectDataOptions::new().with_owner()),
            )
            .await
            .unwrap();
        assert_eq!(
            transferred_object.owner(),
            Some(Owner::AddressOwner(other_address))
        );
    }

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

    let tx_response = execute_tx(&cluster, http_client, transaction_bytes)
        .await
        .unwrap();

    matches!(tx_response, IotaTransactionBlockResponse {effects, ..} if effects.as_ref().unwrap().created().len() == 1);
    Ok(())
}

async fn execute_tx(
    cluster: &TestCluster,
    http_client: &HttpClient,
    transaction_bytes: TransactionBlockBytes,
) -> Result<IotaTransactionBlockResponse, anyhow::Error> {
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
                    .with_object_changes()
                    .with_balance_changes(),
            ),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await?;
    assert_eq!(tx_response.status_ok(), Some(true));

    Ok(tx_response)
}
