// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::time::Duration;

use iota_json_rpc_types::IotaTransactionBlockResponseOptions;
use iota_keys::keystore::AccountKeystore;
use iota_rosetta::{
    operations::Operations,
    types::{
        AccountBalanceRequest, AccountBalanceResponse, AccountIdentifier, IotaEnv,
        NetworkIdentifier, SubAccount, SubAccountType,
    },
};
use iota_sdk::rpc_types::{IotaExecutionStatus, IotaTransactionBlockEffectsAPI};
use iota_swarm_config::genesis_config::{DEFAULT_GAS_AMOUNT, DEFAULT_NUMBER_OF_OBJECT_PER_ACCOUNT};
use iota_types::{
    iota_system_state::iota_system_state_summary::IotaSystemStateSummary,
    quorum_driver_types::ExecuteTransactionRequestType, utils::to_sender_signed_transaction,
};
use rosetta_client::start_rosetta_test_server;
use serde_json::json;
use test_cluster::TestClusterBuilder;

use crate::rosetta_client::RosettaEndpoint;

mod rosetta_client;

#[tokio::test]
async fn test_get_staked_iota() {
    let test_cluster = TestClusterBuilder::new().build().await;
    let address = test_cluster.get_address_0();
    let client = test_cluster.wallet.get_client().await.unwrap();
    let keystore = test_cluster.wallet.config().keystore();

    let (rosetta_client, _handle) = start_rosetta_test_server(client.clone()).await;

    tokio::time::sleep(Duration::from_secs(1)).await;

    let network_identifier = NetworkIdentifier {
        blockchain: "iota".to_string(),
        network: IotaEnv::LocalNet,
    };
    // Verify initial balance and stake
    let request = AccountBalanceRequest {
        network_identifier: network_identifier.clone(),
        account_identifier: AccountIdentifier {
            address,
            sub_account: None,
        },
        block_identifier: Default::default(),
        currencies: vec![],
    };

    let response: AccountBalanceResponse = rosetta_client
        .call(RosettaEndpoint::Balance, &request)
        .await;
    assert_eq!(1, response.balances.len());
    assert_eq!(
        (DEFAULT_GAS_AMOUNT * DEFAULT_NUMBER_OF_OBJECT_PER_ACCOUNT as u64) as i128,
        response.balances[0].value
    );

    let request = AccountBalanceRequest {
        network_identifier: network_identifier.clone(),
        account_identifier: AccountIdentifier {
            address,
            sub_account: Some(SubAccount {
                account_type: SubAccountType::PendingStake,
            }),
        },
        block_identifier: Default::default(),
        currencies: vec![],
    };
    let response: AccountBalanceResponse = rosetta_client
        .call(RosettaEndpoint::Balance, &request)
        .await;
    assert_eq!(response.balances[0].value, 0);

    // Stake some iota
    let system_state = client
        .governance_api()
        .get_latest_iota_system_state()
        .await
        .unwrap();
    let active_validators = match system_state {
        IotaSystemStateSummary::V1(v1) => v1.active_validators,
        IotaSystemStateSummary::V2(v2) => v2.active_validators,
        _ => panic!("unsupported IotaSystemStateSummary"),
    };
    let validator = active_validators[0].iota_address;
    let coins = client
        .coin_read_api()
        .get_coins(address, None, None, None)
        .await
        .unwrap()
        .data;
    let delegation_tx = client
        .transaction_builder()
        .request_add_stake(
            address,
            vec![coins[0].coin_object_id],
            Some(1_000_000_000),
            validator,
            None,
            1_000_000_000,
        )
        .await
        .unwrap();
    let tx = to_sender_signed_transaction(delegation_tx, keystore.get_key(&address).unwrap());
    client
        .quorum_driver_api()
        .execute_transaction_block(
            tx,
            IotaTransactionBlockResponseOptions::new(),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await
        .unwrap();

    let response = rosetta_client
        .get_balance(
            network_identifier.clone(),
            address,
            Some(SubAccountType::PendingStake),
        )
        .await;
    assert_eq!(1, response.balances.len());
    assert_eq!(1_000_000_000, response.balances[0].value);

    println!("{}", serde_json::to_string_pretty(&response).unwrap());
}

#[tokio::test]
async fn test_stake() {
    let test_cluster = TestClusterBuilder::new().build().await;
    let sender = test_cluster.get_address_0();
    let client = test_cluster.wallet.get_client().await.unwrap();
    let keystore = test_cluster.wallet.config().keystore();

    let (rosetta_client, _handle) = start_rosetta_test_server(client.clone()).await;

    let system_state = client
        .governance_api()
        .get_latest_iota_system_state()
        .await
        .unwrap();
    let active_validators = match system_state {
        IotaSystemStateSummary::V1(v1) => v1.active_validators,
        IotaSystemStateSummary::V2(v2) => v2.active_validators,
        _ => panic!("unsupported IotaSystemStateSummary"),
    };
    let validator = active_validators[0].iota_address;

    let ops = serde_json::from_value(json!(
        [{
            "operation_identifier":{"index":0},
            "type":"Stake",
            "account": { "address" : sender.to_string() },
            "amount" : { "value": "-1000000000" , "currency": { "symbol": "IOTA", "decimals": 9}},
            "metadata": { "Stake" : {"validator": validator.to_string()} }
        }]
    ))
    .unwrap();

    let response = rosetta_client.rosetta_flow(&ops, keystore).await;

    let tx = client
        .read_api()
        .get_transaction_with_options(
            response.transaction_identifier.hash,
            IotaTransactionBlockResponseOptions::new()
                .with_input()
                .with_effects()
                .with_balance_changes()
                .with_events(),
        )
        .await
        .unwrap();

    println!("IOTA TX: {tx:?}");

    assert_eq!(
        &IotaExecutionStatus::Success,
        tx.effects.as_ref().unwrap().status()
    );

    let ops2 = Operations::try_from(tx).unwrap();
    assert!(
        ops2.contains(&ops),
        "Operation mismatch. expecting:{}, got:{}",
        serde_json::to_string(&ops).unwrap(),
        serde_json::to_string(&ops2).unwrap()
    );

    println!("{}", serde_json::to_string_pretty(&ops2).unwrap())
}

#[tokio::test]
async fn test_stake_all() {
    let test_cluster = TestClusterBuilder::new().build().await;
    let sender = test_cluster.get_address_0();
    let client = test_cluster.wallet.get_client().await.unwrap();
    let keystore = test_cluster.wallet.config().keystore();

    let (rosetta_client, _handle) = start_rosetta_test_server(client.clone()).await;

    let system_state = client
        .governance_api()
        .get_latest_iota_system_state()
        .await
        .unwrap();
    let active_validators = match system_state {
        IotaSystemStateSummary::V1(v1) => v1.active_validators,
        IotaSystemStateSummary::V2(v2) => v2.active_validators,
        _ => panic!("unsupported IotaSystemStateSummary"),
    };
    let validator = active_validators[0].iota_address;

    let ops = serde_json::from_value(json!(
        [{
            "operation_identifier":{"index":0},
            "type":"Stake",
            "account": { "address" : sender.to_string() },
            "metadata": { "Stake" : {"validator": validator.to_string()} }
        }]
    ))
    .unwrap();

    let response = rosetta_client.rosetta_flow(&ops, keystore).await;

    let tx = client
        .read_api()
        .get_transaction_with_options(
            response.transaction_identifier.hash,
            IotaTransactionBlockResponseOptions::new()
                .with_input()
                .with_effects()
                .with_balance_changes()
                .with_events(),
        )
        .await
        .unwrap();

    println!("IOTA TX: {tx:?}");

    assert_eq!(
        &IotaExecutionStatus::Success,
        tx.effects.as_ref().unwrap().status()
    );

    let ops2 = Operations::try_from(tx).unwrap();
    assert!(
        ops2.contains(&ops),
        "Operation mismatch. expecting:{}, got:{}",
        serde_json::to_string(&ops).unwrap(),
        serde_json::to_string(&ops2).unwrap()
    );

    println!("{}", serde_json::to_string_pretty(&ops2).unwrap())
}

#[tokio::test]
async fn test_withdraw_stake() {
    telemetry_subscribers::init_for_testing();

    let test_cluster = TestClusterBuilder::new()
        .with_epoch_duration_ms(15000)
        .build()
        .await;
    let sender = test_cluster.get_address_0();
    let client = test_cluster.wallet.get_client().await.unwrap();
    let keystore = test_cluster.wallet.config().keystore();

    let (rosetta_client, _handle) = start_rosetta_test_server(client.clone()).await;

    // First add some stakes
    let system_state = client
        .governance_api()
        .get_latest_iota_system_state()
        .await
        .unwrap();
    let active_validators = match system_state {
        IotaSystemStateSummary::V1(v1) => v1.active_validators,
        IotaSystemStateSummary::V2(v2) => v2.active_validators,
        _ => panic!("unsupported IotaSystemStateSummary"),
    };
    let validator = active_validators[0].iota_address;

    let ops = serde_json::from_value(json!(
        [{
            "operation_identifier":{"index":0},
            "type":"Stake",
            "account": { "address" : sender.to_string() },
            "amount" : { "value": "-1000000000" , "currency": { "symbol": "IOTA", "decimals": 9}},
            "metadata": { "Stake" : {"validator": validator.to_string()} }
        }]
    ))
    .unwrap();

    let response = rosetta_client.rosetta_flow(&ops, keystore).await;

    let tx = client
        .read_api()
        .get_transaction_with_options(
            response.transaction_identifier.hash,
            IotaTransactionBlockResponseOptions::new()
                .with_input()
                .with_effects()
                .with_balance_changes()
                .with_events(),
        )
        .await
        .unwrap();

    println!("IOTA TX: {tx:?}");

    assert_eq!(
        &IotaExecutionStatus::Success,
        tx.effects.as_ref().unwrap().status()
    );
    // verify balance
    let network_identifier = NetworkIdentifier {
        blockchain: "iota".to_string(),
        network: IotaEnv::LocalNet,
    };
    let response = rosetta_client
        .get_balance(
            network_identifier.clone(),
            sender,
            Some(SubAccountType::PendingStake),
        )
        .await;

    assert_eq!(1, response.balances.len());
    assert_eq!(1000000000, response.balances[0].value);

    // wait for epoch.
    tokio::time::sleep(Duration::from_millis(15000)).await;

    // withdraw all stake
    let ops = serde_json::from_value(json!(
        [{
            "operation_identifier":{"index":0},
            "type":"WithdrawStake",
            "account": { "address" : sender.to_string() }
        }]
    ))
    .unwrap();

    let response = rosetta_client.rosetta_flow(&ops, keystore).await;

    let tx = client
        .read_api()
        .get_transaction_with_options(
            response.transaction_identifier.hash,
            IotaTransactionBlockResponseOptions::new()
                .with_input()
                .with_effects()
                .with_balance_changes()
                .with_events(),
        )
        .await
        .unwrap();

    assert_eq!(
        &IotaExecutionStatus::Success,
        tx.effects.as_ref().unwrap().status()
    );
    println!("IOTA TX: {tx:?}");

    let ops2 = Operations::try_from(tx).unwrap();
    assert!(
        ops2.contains(&ops),
        "Operation mismatch. expecting:{}, got:{}",
        serde_json::to_string(&ops).unwrap(),
        serde_json::to_string(&ops2).unwrap()
    );

    println!("{}", serde_json::to_string_pretty(&ops2).unwrap());

    // stake should be 0
    let response = rosetta_client
        .get_balance(
            network_identifier.clone(),
            sender,
            Some(SubAccountType::PendingStake),
        )
        .await;

    assert_eq!(1, response.balances.len());
    assert_eq!(0, response.balances[0].value);
}

#[tokio::test]
async fn test_pay_iota() {
    let test_cluster = TestClusterBuilder::new().build().await;
    let sender = test_cluster.get_address_0();
    let recipient = test_cluster.get_address_1();
    let client = test_cluster.wallet.get_client().await.unwrap();
    let keystore = test_cluster.wallet.config().keystore();

    let (rosetta_client, _handle) = start_rosetta_test_server(client.clone()).await;

    let ops = serde_json::from_value(json!(
        [{
            "operation_identifier":{"index":0},
            "type":"PayIota",
            "account": { "address" : recipient.to_string() },
            "amount" : { "value": "1000000000" , "currency": { "symbol": "IOTA", "decimals": 9}}
        },{
            "operation_identifier":{"index":1},
            "type":"PayIota",
            "account": { "address" : sender.to_string() },
            "amount" : { "value": "-1000000000" , "currency": { "symbol": "IOTA", "decimals": 9}}
        }]
    ))
    .unwrap();

    let response = rosetta_client.rosetta_flow(&ops, keystore).await;

    let tx = client
        .read_api()
        .get_transaction_with_options(
            response.transaction_identifier.hash,
            IotaTransactionBlockResponseOptions::new()
                .with_input()
                .with_effects()
                .with_balance_changes()
                .with_events(),
        )
        .await
        .unwrap();

    assert_eq!(
        &IotaExecutionStatus::Success,
        tx.effects.as_ref().unwrap().status()
    );
    println!("IOTA TX: {tx:?}");

    let ops2 = Operations::try_from(tx).unwrap();
    assert!(
        ops2.contains(&ops),
        "Operation mismatch. expecting:{}, got:{}",
        serde_json::to_string(&ops).unwrap(),
        serde_json::to_string(&ops2).unwrap()
    );
}

#[tokio::test]
async fn test_pay_iota_multiple_times() {
    let test_cluster = TestClusterBuilder::new()
        .with_epoch_duration_ms(36000000)
        .build()
        .await;
    let sender = test_cluster.get_address_0();
    let recipient = test_cluster.get_address_1();
    let client = test_cluster.wallet.get_client().await.unwrap();
    let keystore = test_cluster.wallet.config().keystore();

    let (rosetta_client, _handle) = start_rosetta_test_server(client.clone()).await;

    for i in 1..20 {
        println!("Iteration: {}", i);
        let ops = serde_json::from_value(json!(
            [{
                "operation_identifier":{"index":0},
                "type":"PayIota",
                "account": { "address" : recipient.to_string() },
                "amount" : { "value": "1000000000" , "currency": { "symbol": "IOTA", "decimals": 9}}
            },{
                "operation_identifier":{"index":1},
                "type":"PayIota",
                "account": { "address" : sender.to_string() },
                "amount" : { "value": "-1000000000" , "currency": { "symbol": "IOTA", "decimals": 9}}
            }]
        ))
        .unwrap();

        let response = rosetta_client.rosetta_flow(&ops, keystore).await;

        let tx = client
            .read_api()
            .get_transaction_with_options(
                response.transaction_identifier.hash,
                IotaTransactionBlockResponseOptions::new()
                    .with_input()
                    .with_effects()
                    .with_balance_changes()
                    .with_events(),
            )
            .await
            .unwrap();
        println!("IOTA TX: {tx:?}");
        assert_eq!(
            &IotaExecutionStatus::Success,
            tx.effects.as_ref().unwrap().status()
        );

        let ops2 = Operations::try_from(tx).unwrap();
        assert!(
            ops2.contains(&ops),
            "Operation mismatch. expecting:{}, got:{}",
            serde_json::to_string(&ops).unwrap(),
            serde_json::to_string(&ops2).unwrap()
        );
    }
}
