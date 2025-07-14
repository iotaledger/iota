// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::str::FromStr;

use iota_json::{call_args, type_args};
use iota_json_rpc_api::{
    CoinReadApiClient, GovernanceReadApiClient, ReadApiClient, TransactionBuilderClient,
};
use iota_json_rpc_types::{
    IotaObjectDataOptions, MoveCallParams, RPCTransactionRequestParams, StakeStatus,
    TransactionBlockBytes, TransferObjectParams,
};
use iota_types::{
    IOTA_FRAMEWORK_ADDRESS,
    base_types::{IotaAddress, ObjectID},
    crypto::{AccountKeyPair, get_key_pair},
    gas_coin::GAS,
    object::Owner,
};
use jsonrpsee::http_client::HttpClient;

use crate::common::{
    ApiTestSetup, execute_tx_and_wait_for_indexer,
    rpc_tests::{create_coins_and_wait_for_indexer, get_validator},
};

const FUNDED_BALANCE_PER_COIN: u64 = 10_000_000_000;

#[test]
fn transfer_object() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        cluster,
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        let (sender, keypair): (_, AccountKeyPair) = get_key_pair();
        let (receiver, _): (_, AccountKeyPair) = get_key_pair();

        let sender_coins = create_coins_and_wait_for_indexer(cluster, client, sender, 2).await;
        let gas = sender_coins[0];
        let object_to_send = sender_coins[1];

        let tx_bytes = client
            .transfer_object(
                sender,
                object_to_send,
                Some(gas),
                100_000_000.into(),
                receiver,
            )
            .await
            .unwrap();
        execute_tx_and_wait_for_indexer(client, cluster, store, tx_bytes, &keypair).await;

        let transferred_object = client
            .get_object(object_to_send, Some(IotaObjectDataOptions::full_content()))
            .await
            .unwrap();

        assert_eq!(
            transferred_object.owner(),
            Some(Owner::AddressOwner(receiver))
        );
    });
}

#[test]
fn transfer_iota() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        cluster,
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        let (sender, keypair): (_, AccountKeyPair) = get_key_pair();
        let (receiver, _): (_, AccountKeyPair) = get_key_pair();

        let sender_coins = create_coins_and_wait_for_indexer(cluster, client, sender, 1).await;
        let gas = sender_coins[0];
        let transferred_balance = 100_000;

        let tx_bytes = client
            .transfer_iota(
                sender,
                gas,
                100_000_000.into(),
                receiver,
                Some(transferred_balance.into()),
            )
            .await
            .unwrap();
        execute_tx_and_wait_for_indexer(client, cluster, store, tx_bytes, &keypair).await;

        let receiver_balances = get_address_balances(client, receiver).await;

        assert_eq!(receiver_balances, [transferred_balance]);
    });
}

#[test]
fn pay() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        cluster,
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        let (sender, keypair): (_, AccountKeyPair) = get_key_pair();
        let (receiver_1, _): (_, AccountKeyPair) = get_key_pair();
        let (receiver_2, _): (_, AccountKeyPair) = get_key_pair();

        let input_coins: u64 = 3;
        let sender_coins =
            create_coins_and_wait_for_indexer(cluster, client, sender, input_coins as u32 + 1)
                .await;
        let total_input_coins_balance = FUNDED_BALANCE_PER_COIN * input_coins;
        let transferred_balance_1 = total_input_coins_balance / 2 - 100;
        let transferred_balance_2 = total_input_coins_balance / 2 - 700;

        let tx_bytes = client
            .pay(
                sender,
                sender_coins[0..input_coins as usize].into(),
                [receiver_1, receiver_2].into(),
                [transferred_balance_1.into(), transferred_balance_2.into()].into(),
                None, // let node find the gas automatically
                100_000_000.into(),
            )
            .await
            .unwrap();
        execute_tx_and_wait_for_indexer(client, cluster, store, tx_bytes, &keypair).await;

        let receiver_1_balances = get_address_balances(client, receiver_1).await;
        let receiver_2_balances = get_address_balances(client, receiver_2).await;

        assert_eq!(receiver_1_balances, [transferred_balance_1]);
        assert_eq!(receiver_2_balances, [transferred_balance_2]);
    });
}

#[test]
fn pay_iota() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        cluster,
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        let (sender, keypair): (_, AccountKeyPair) = get_key_pair();
        let (receiver_1, _): (_, AccountKeyPair) = get_key_pair();
        let (receiver_2, _): (_, AccountKeyPair) = get_key_pair();

        let input_coins: u64 = 3;
        let sender_coins =
            create_coins_and_wait_for_indexer(cluster, client, sender, input_coins as u32).await;
        let gas_budget = 100_000_000;
        let total_available_input_coins_balance: u64 =
            FUNDED_BALANCE_PER_COIN * input_coins - gas_budget;
        let transferred_balance_1 = total_available_input_coins_balance / 2 - 100;
        let transferred_balance_2 = total_available_input_coins_balance / 2 - 700;

        let tx_bytes = client
            .pay_iota(
                sender,
                sender_coins,
                [receiver_1, receiver_2].into(),
                [transferred_balance_1.into(), transferred_balance_2.into()].into(),
                gas_budget.into(),
            )
            .await
            .unwrap();
        execute_tx_and_wait_for_indexer(client, cluster, store, tx_bytes, &keypair).await;

        let receiver_1_balances = get_address_balances(client, receiver_1).await;
        let receiver_2_balances = get_address_balances(client, receiver_2).await;

        assert_eq!(receiver_1_balances, [transferred_balance_1]);
        assert_eq!(receiver_2_balances, [transferred_balance_2]);
    });
}

#[test]
fn pay_all_iota() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        cluster,
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        let (sender, keypair): (_, AccountKeyPair) = get_key_pair();
        let (receiver, _): (_, AccountKeyPair) = get_key_pair();

        let input_coins: u64 = 3;
        let sender_coins =
            create_coins_and_wait_for_indexer(cluster, client, sender, input_coins as u32).await;
        let gas_budget = 100_000_000;

        let tx_bytes = client
            .pay_all_iota(sender, sender_coins, receiver, gas_budget.into())
            .await
            .unwrap();
        execute_tx_and_wait_for_indexer(client, cluster, store, tx_bytes, &keypair).await;

        let receiver_balances = get_address_balances(client, receiver).await;
        let expected_minimum_receiver_balance = FUNDED_BALANCE_PER_COIN * input_coins - gas_budget;

        assert_eq!(receiver_balances.len(), 1);
        assert!(receiver_balances[0] >= expected_minimum_receiver_balance);
    });
}

#[test]
fn move_call() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        cluster,
    } = ApiTestSetup::get_or_init();

    runtime
        .block_on(async move {
            let (sender, keypair): (_, AccountKeyPair) = get_key_pair();

            let sender_coins = create_coins_and_wait_for_indexer(cluster, client, sender, 3).await;
            let gas = sender_coins[0];

            let tx_bytes = client
                .move_call(
                    sender,
                    ObjectID::new(IOTA_FRAMEWORK_ADDRESS.into_bytes()),
                    "coin".to_string(),
                    "join".to_string(),
                    type_args![GAS::type_tag()].unwrap(),
                    call_args!(sender_coins[1], sender_coins[2]).unwrap(),
                    Some(gas),
                    10_000_000.into(),
                    None,
                )
                .await
                .unwrap();
            execute_tx_and_wait_for_indexer(client, cluster, store, tx_bytes, &keypair).await;

            let mut sender_balances = get_address_balances(client, sender).await;
            sender_balances.sort();

            assert_eq!(sender_balances[1], FUNDED_BALANCE_PER_COIN * 2);

            Ok::<(), anyhow::Error>(())
        })
        .unwrap();
}

#[test]
fn split_coin() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        cluster,
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        let (sender, keypair): (_, AccountKeyPair) = get_key_pair();

        let sender_coins = create_coins_and_wait_for_indexer(cluster, client, sender, 2).await;
        let split_amount_1 = 100_000;
        let split_amount_2 = 20_000;
        let split_amount_3 = 30_000;
        let gas_budget = 100_000_000;

        let tx_bytes = client
            .split_coin(
                sender,
                sender_coins[0],
                vec![
                    split_amount_1.into(),
                    split_amount_2.into(),
                    split_amount_3.into(),
                ],
                None,
                gas_budget.into(),
            )
            .await
            .unwrap();
        execute_tx_and_wait_for_indexer(client, cluster, store, tx_bytes, &keypair).await;

        let mut sender_balances = get_address_balances(client, sender).await;
        sender_balances.sort();

        assert_eq!(
            sender_balances[0..3],
            [split_amount_2, split_amount_3, split_amount_1,]
        );
    });
}

#[test]
fn split_coin_equal() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        cluster,
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        let (sender, keypair): (_, AccountKeyPair) = get_key_pair();

        let sender_coins = create_coins_and_wait_for_indexer(cluster, client, sender, 2).await;
        let gas_budget = 100_000_000;

        let tx_bytes = client
            .split_coin_equal(sender, sender_coins[0], 3.into(), None, gas_budget.into())
            .await
            .unwrap();
        execute_tx_and_wait_for_indexer(client, cluster, store, tx_bytes, &keypair).await;

        let mut sender_balances = get_address_balances(client, sender).await;
        sender_balances.sort();

        assert_eq!(
            sender_balances[0..3],
            [3_333_333_333, 3_333_333_333, 3_333_333_334,]
        );
    });
}

#[test]
fn merge_coin() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        cluster,
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        let (sender, keypair): (_, AccountKeyPair) = get_key_pair();

        let sender_coins = create_coins_and_wait_for_indexer(cluster, client, sender, 3).await;
        let gas_budget = 100_000_000;

        let tx_bytes = client
            .merge_coin(
                sender,
                sender_coins[0],
                sender_coins[1],
                Some(sender_coins[2]),
                gas_budget.into(),
            )
            .await
            .unwrap();
        execute_tx_and_wait_for_indexer(client, cluster, store, tx_bytes, &keypair).await;

        let mut sender_balances = get_address_balances(client, sender).await;
        sender_balances.sort();

        assert_eq!(sender_balances.len(), 2);
        assert_eq!(sender_balances[1], FUNDED_BALANCE_PER_COIN * 2);
    });
}

#[test]
fn batch_transaction() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        cluster,
    } = ApiTestSetup::get_or_init();

    runtime
        .block_on(async move {
            let (sender, keypair): (_, AccountKeyPair) = get_key_pair();
            let (receiver, _): (_, AccountKeyPair) = get_key_pair();

            let sender_coins = create_coins_and_wait_for_indexer(cluster, client, sender, 3).await;
            let gas = sender_coins[0];
            let coin_to_split = sender_coins[1];
            let coin_to_transfer: ObjectID = sender_coins[2];
            let amount_to_split = FUNDED_BALANCE_PER_COIN / 2 - 123_000;
            let amount_to_leave = FUNDED_BALANCE_PER_COIN - amount_to_split;

            let tx_bytes: TransactionBlockBytes = client
                .batch_transaction(
                    sender,
                    vec![
                        RPCTransactionRequestParams::MoveCallRequestParams(MoveCallParams {
                            package_object_id: ObjectID::new(IOTA_FRAMEWORK_ADDRESS.into_bytes()),
                            module: "pay".to_string(),
                            function: "split".to_string(),
                            type_arguments: type_args![GAS::type_tag()]?,
                            arguments: call_args!(coin_to_split, amount_to_split)?,
                        }),
                        RPCTransactionRequestParams::TransferObjectRequestParams(
                            TransferObjectParams {
                                recipient: receiver,
                                object_id: coin_to_transfer,
                            },
                        ),
                    ],
                    Some(gas),
                    10_000_000.into(),
                    None,
                )
                .await?;
            execute_tx_and_wait_for_indexer(client, cluster, store, tx_bytes, &keypair).await;

            let mut sender_balances = get_address_balances(client, sender).await;
            let receiver_balances = get_address_balances(client, receiver).await;
            sender_balances.sort();

            assert_eq!(sender_balances.len(), 3);
            assert_eq!(sender_balances[0..2], [amount_to_split, amount_to_leave]);
            assert_eq!(receiver_balances, [FUNDED_BALANCE_PER_COIN]);

            Ok::<(), anyhow::Error>(())
        })
        .unwrap();
}

#[test]
fn request_withdraw_stake_from_pending() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        cluster,
    } = ApiTestSetup::get_or_init();

    runtime
        .block_on(async move {
            let (address, keypair): (_, AccountKeyPair) = get_key_pair();
            let coins = create_coins_and_wait_for_indexer(cluster, client, address, 4).await;
            let gas = coins[3];
            let coins_to_stake = coins[..3].to_vec();
            let validator = get_validator(client).await;
            // subtracting some amount to see if it is possible to stake smaller amount than
            // is provided in the input coins
            let stake_amount = FUNDED_BALANCE_PER_COIN * 3 - 10_000;

            let tx_bytes: TransactionBlockBytes = client
                .request_add_stake(
                    address,
                    coins_to_stake,
                    Some(stake_amount.into()),
                    validator,
                    Some(gas),
                    100_000_000.into(),
                )
                .await
                .unwrap();
            execute_tx_and_wait_for_indexer(client, cluster, store, tx_bytes, &keypair).await;

            let staked_iota = client.get_stakes(address).await.unwrap();
            let stake = &staked_iota[0].stakes[0];
            assert!(matches!(stake.status, StakeStatus::Pending));

            let tx_bytes: TransactionBlockBytes = client
                .request_withdraw_stake(
                    address,
                    stake.staked_iota_id,
                    Some(gas),
                    100_000_000.into(),
                )
                .await
                .unwrap();
            execute_tx_and_wait_for_indexer(client, cluster, store, tx_bytes, &keypair).await;

            let staked_iota = client.get_stakes(address).await.unwrap();
            assert!(staked_iota.is_empty());

            Ok::<(), anyhow::Error>(())
        })
        .unwrap();
}

async fn get_address_balances(indexer_client: &HttpClient, address: IotaAddress) -> Vec<u64> {
    indexer_client
        .get_coins(address, None, None, None)
        .await
        .unwrap()
        .data
        .iter()
        .map(|coin| coin.balance)
        .collect()
}
