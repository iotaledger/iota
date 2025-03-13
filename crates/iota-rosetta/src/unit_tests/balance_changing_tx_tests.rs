// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
    str::FromStr,
};

use anyhow::anyhow;
use iota_json_rpc_types::{
    IotaObjectDataOptions, IotaObjectRef, IotaObjectResponseQuery,
    IotaTransactionBlockResponseOptions, ObjectChange,
};
use iota_keys::keystore::{AccountKeystore, Keystore};
use iota_move_build::BuildConfig;
use iota_sdk::{
    IotaClient,
    rpc_types::{
        IotaData, IotaExecutionStatus, IotaTransactionBlockEffectsAPI,
        IotaTransactionBlockResponse, OwnedObjectRef,
    },
};
use iota_types::{
    TypeTag,
    base_types::{IotaAddress, ObjectID, ObjectRef},
    gas_coin::GasCoin,
    iota_system_state::iota_system_state_summary::IotaSystemStateSummary,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    quorum_driver_types::ExecuteTransactionRequestType,
    transaction::{
        CallArg, InputObjectKind, ObjectArg, ProgrammableTransaction,
        TEST_ONLY_GAS_UNIT_FOR_GENERIC, TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE,
        TEST_ONLY_GAS_UNIT_FOR_SPLIT_COIN, TEST_ONLY_GAS_UNIT_FOR_STAKING,
        TEST_ONLY_GAS_UNIT_FOR_TRANSFER, Transaction, TransactionData, TransactionDataAPI,
        TransactionKind,
    },
};
use move_core_types::{identifier::Identifier, language_storage::StructTag};
use rand::seq::{IteratorRandom, SliceRandom};
use serde_json::json;
use shared_crypto::intent::Intent;
use signature::rand_core::OsRng;
use test_cluster::TestClusterBuilder;

use crate::{
    operations::Operations,
    types::{ConstructionMetadata, OperationStatus, OperationType},
};

#[tokio::test]
async fn test_transfer_iota() {
    let network = TestClusterBuilder::new().build().await;
    let client = network.wallet.get_client().await.unwrap();
    let keystore = network.wallet.config().keystore();
    let rgp = network.get_reference_gas_price().await;

    // Test Transfer IOTA
    let addresses = network.get_addresses();
    let sender = get_random_address(&addresses, vec![]);
    let recipient = get_random_address(&addresses, vec![sender]);
    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();
        builder.transfer_iota(recipient, Some(50000));
        builder.finish()
    };
    test_transaction(
        &client,
        keystore,
        vec![recipient],
        sender,
        pt,
        vec![],
        rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
        rgp,
        false,
    )
    .await;
}

#[tokio::test]
async fn test_transfer_iota_whole_coin() {
    let network = TestClusterBuilder::new().build().await;
    let client = network.wallet.get_client().await.unwrap();
    let keystore = network.wallet.config().keystore();
    let rgp = network.get_reference_gas_price().await;

    // Test transfer iota whole coin
    let addresses = network.get_addresses();
    let sender = get_random_address(&addresses, vec![]);
    let recipient = get_random_address(&addresses, vec![sender]);
    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();
        builder.transfer_iota(recipient, None);
        builder.finish()
    };
    test_transaction(
        &client,
        keystore,
        vec![recipient],
        sender,
        pt,
        vec![],
        rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
        rgp,
        false,
    )
    .await;
}

#[tokio::test]
async fn test_transfer_object() {
    let network = TestClusterBuilder::new().build().await;
    let client = network.wallet.get_client().await.unwrap();
    let keystore = network.wallet.config().keystore();
    let rgp = network.get_reference_gas_price().await;

    // Test transfer object
    let addresses = network.get_addresses();
    let sender = get_random_address(&addresses, vec![]);
    let recipient = get_random_address(&addresses, vec![sender]);
    let object_ref = get_random_iota(&client, sender, vec![]).await;
    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();
        builder.transfer_object(recipient, object_ref).unwrap();
        builder.finish()
    };
    test_transaction(
        &client,
        keystore,
        vec![recipient],
        sender,
        pt,
        vec![],
        rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
        rgp,
        false,
    )
    .await;
}

#[tokio::test]
async fn test_publish_and_move_call() {
    let network = TestClusterBuilder::new().build().await;
    let client = network.wallet.get_client().await.unwrap();
    let keystore = network.wallet.config().keystore();
    let rgp = network.get_reference_gas_price().await;

    // Test publish
    let addresses = network.get_addresses();
    let sender = get_random_address(&addresses, vec![]);
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.extend(["..", "..", "examples", "move", "coin"]);
    let compiled_package = BuildConfig::new_for_testing().build(&path).unwrap();
    let compiled_modules_bytes =
        compiled_package.get_package_bytes(/* with_unpublished_deps */ false);
    let dependencies = compiled_package.get_dependency_storage_package_ids();

    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();
        builder.publish_immutable(compiled_modules_bytes, dependencies);
        builder.finish()
    };
    let response = test_transaction(
        &client,
        keystore,
        vec![],
        sender,
        pt,
        vec![],
        rgp * TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE,
        rgp,
        false,
    )
    .await;
    let object_changes = response.object_changes.unwrap();

    // Test move call (reuse published module from above test)
    let package = object_changes
        .iter()
        .find_map(|change| {
            if let ObjectChange::Published { package_id, .. } = change {
                Some(package_id)
            } else {
                None
            }
        })
        .unwrap();

    let treasury = find_module_object(&object_changes, |type_| {
        if type_.name.as_str() != "TreasuryCap" {
            return false;
        }

        let Some(TypeTag::Struct(otw)) = type_.type_params.first() else {
            return false;
        };

        otw.name.as_str() == "MY_COIN"
    });

    let treasury = treasury.clone().reference.to_object_ref();
    let recipient = *addresses.choose(&mut OsRng).unwrap();
    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();
        builder
            .move_call(
                *package,
                Identifier::from_str("my_coin").unwrap(),
                Identifier::from_str("mint").unwrap(),
                vec![],
                vec![
                    CallArg::Object(ObjectArg::ImmOrOwnedObject(treasury)),
                    CallArg::Pure(bcs::to_bytes(&10000u64).unwrap()),
                    CallArg::Pure(bcs::to_bytes(&recipient).unwrap()),
                ],
            )
            .unwrap();
        builder.finish()
    };

    test_transaction(
        &client,
        keystore,
        vec![],
        sender,
        pt,
        vec![],
        rgp * TEST_ONLY_GAS_UNIT_FOR_GENERIC,
        rgp,
        false,
    )
    .await;
}

#[tokio::test]
async fn test_split_coin() {
    let network = TestClusterBuilder::new().build().await;
    let client = network.wallet.get_client().await.unwrap();
    let keystore = network.wallet.config().keystore();
    let rgp = network.get_reference_gas_price().await;

    // Test spilt coin
    let sender = get_random_address(&network.get_addresses(), vec![]);
    let coin = get_random_iota(&client, sender, vec![]).await;
    let tx = client
        .transaction_builder()
        .split_coin(sender, coin.0, vec![100000], None, 10000)
        .await
        .unwrap();
    let pt = match tx.into_kind() {
        TransactionKind::ProgrammableTransaction(pt) => pt,
        _ => unreachable!(),
    };
    test_transaction(
        &client,
        keystore,
        vec![],
        sender,
        pt,
        vec![],
        rgp * TEST_ONLY_GAS_UNIT_FOR_SPLIT_COIN,
        rgp,
        false,
    )
    .await;
}

#[tokio::test]
async fn test_merge_coin() {
    let network = TestClusterBuilder::new().build().await;
    let client = network.wallet.get_client().await.unwrap();
    let keystore = network.wallet.config().keystore();
    let rgp = network.get_reference_gas_price().await;

    // Test merge coin
    let sender = get_random_address(&network.get_addresses(), vec![]);
    let coin = get_random_iota(&client, sender, vec![]).await;
    let coin2 = get_random_iota(&client, sender, vec![coin.0]).await;
    let tx = client
        .transaction_builder()
        .merge_coins(sender, coin.0, coin2.0, None, 10000)
        .await
        .unwrap();
    let pt = match tx.into_kind() {
        TransactionKind::ProgrammableTransaction(pt) => pt,
        _ => unreachable!(),
    };
    test_transaction(
        &client,
        keystore,
        vec![],
        sender,
        pt,
        vec![],
        rgp * TEST_ONLY_GAS_UNIT_FOR_GENERIC,
        rgp,
        false,
    )
    .await;
}

#[tokio::test]
async fn test_pay() {
    let network = TestClusterBuilder::new().build().await;
    let client = network.wallet.get_client().await.unwrap();
    let keystore = network.wallet.config().keystore();
    let rgp = network.get_reference_gas_price().await;

    // Test Pay
    let addresses = network.get_addresses();
    let sender = get_random_address(&addresses, vec![]);
    let recipient = get_random_address(&addresses, vec![sender]);
    let coin = get_random_iota(&client, sender, vec![]).await;
    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();
        builder
            .pay(vec![coin], vec![recipient], vec![100000])
            .unwrap();
        builder.finish()
    };
    test_transaction(
        &client,
        keystore,
        vec![recipient],
        sender,
        pt,
        vec![],
        rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
        rgp,
        false,
    )
    .await;
}

#[tokio::test]
async fn test_pay_multiple_coin_multiple_recipient() {
    let network = TestClusterBuilder::new().build().await;
    let client = network.wallet.get_client().await.unwrap();
    let keystore = network.wallet.config().keystore();
    let rgp = network.get_reference_gas_price().await;

    // Test Pay multiple coin multiple recipient
    let addresses = network.get_addresses();
    let sender = get_random_address(&addresses, vec![]);
    let recipient1 = get_random_address(&addresses, vec![sender]);
    let recipient2 = get_random_address(&addresses, vec![sender, recipient1]);
    let coin1 = get_random_iota(&client, sender, vec![]).await;
    let coin2 = get_random_iota(&client, sender, vec![coin1.0]).await;
    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();
        builder
            .pay(
                vec![coin1, coin2],
                vec![recipient1, recipient2],
                vec![100000, 200000],
            )
            .unwrap();
        builder.finish()
    };
    test_transaction(
        &client,
        keystore,
        vec![recipient1, recipient2],
        sender,
        pt,
        vec![],
        rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
        rgp,
        false,
    )
    .await;
}

#[tokio::test]
async fn test_pay_iota_multiple_coin_same_recipient() {
    let network = TestClusterBuilder::new().build().await;
    let client = network.wallet.get_client().await.unwrap();
    let keystore = network.wallet.config().keystore();
    let rgp = network.get_reference_gas_price().await;

    // Test Pay multiple coin same recipient
    let addresses = network.get_addresses();
    let sender = get_random_address(&addresses, vec![]);
    let recipient1 = get_random_address(&addresses, vec![sender]);
    let coin1 = get_random_iota(&client, sender, vec![]).await;
    let coin2 = get_random_iota(&client, sender, vec![coin1.0]).await;
    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();
        builder
            .pay_iota(
                vec![recipient1, recipient1, recipient1],
                vec![100000, 100000, 100000],
            )
            .unwrap();
        builder.finish()
    };
    test_transaction(
        &client,
        keystore,
        vec![recipient1],
        sender,
        pt,
        vec![coin1, coin2],
        rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
        rgp,
        false,
    )
    .await;
}

#[tokio::test]
async fn test_pay_iota() {
    let network = TestClusterBuilder::new().build().await;
    let client = network.wallet.get_client().await.unwrap();
    let keystore = network.wallet.config().keystore();
    let rgp = network.get_reference_gas_price().await;

    // Test Pay IOTA
    let addresses = network.get_addresses();
    let sender = get_random_address(&addresses, vec![]);
    let recipient1 = get_random_address(&addresses, vec![sender]);
    let recipient2 = get_random_address(&addresses, vec![sender, recipient1]);
    let coin1 = get_random_iota(&client, sender, vec![]).await;
    let coin2 = get_random_iota(&client, sender, vec![coin1.0]).await;
    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();
        builder
            .pay_iota(vec![recipient1, recipient2], vec![1000000, 2000000])
            .unwrap();
        builder.finish()
    };
    test_transaction(
        &client,
        keystore,
        vec![recipient1, recipient2],
        sender,
        pt,
        vec![coin1, coin2],
        rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
        rgp,
        false,
    )
    .await;
}

#[tokio::test]
async fn test_failed_pay_iota() {
    let network = TestClusterBuilder::new().build().await;
    let client = network.wallet.get_client().await.unwrap();
    let keystore = network.wallet.config().keystore();
    let rgp = network.get_reference_gas_price().await;

    // Test failed Pay IOTA
    let addresses = network.get_addresses();
    let sender = get_random_address(&addresses, vec![]);
    let recipient1 = get_random_address(&addresses, vec![sender]);
    let recipient2 = get_random_address(&addresses, vec![sender, recipient1]);
    let coin1 = get_random_iota(&client, sender, vec![]).await;
    let coin2 = get_random_iota(&client, sender, vec![coin1.0]).await;
    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();
        builder
            .pay_iota(vec![recipient1, recipient2], vec![1000000, 2000000])
            .unwrap();
        builder.finish()
    };
    test_transaction(
        &client,
        keystore,
        vec![],
        sender,
        pt,
        vec![coin1, coin2],
        2000000,
        rgp,
        true,
    )
    .await;
}

#[tokio::test]
async fn test_stake_iota() {
    let network = TestClusterBuilder::new().build().await;
    let client = network.wallet.get_client().await.unwrap();
    let keystore = network.wallet.config().keystore();
    let rgp = network.get_reference_gas_price().await;

    // Test Delegate IOTA
    let sender = get_random_address(&network.get_addresses(), vec![]);
    let coin1 = get_random_iota(&client, sender, vec![]).await;
    let coin2 = get_random_iota(&client, sender, vec![coin1.0]).await;
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
    let tx = client
        .transaction_builder()
        .request_add_stake(
            sender,
            vec![coin1.0, coin2.0],
            Some(1000000000),
            validator,
            None,
            10_000_000,
        )
        .await
        .unwrap();
    let pt = match tx.into_kind() {
        TransactionKind::ProgrammableTransaction(pt) => pt,
        _ => unreachable!(),
    };
    test_transaction(
        &client,
        keystore,
        vec![],
        sender,
        pt,
        vec![],
        rgp * TEST_ONLY_GAS_UNIT_FOR_STAKING,
        rgp,
        false,
    )
    .await;
}

#[tokio::test]
async fn test_stake_iota_with_none_amount() {
    let network = TestClusterBuilder::new().build().await;
    let client = network.wallet.get_client().await.unwrap();
    let keystore = network.wallet.config().keystore();
    let rgp = network.get_reference_gas_price().await;

    // Test Staking IOTA
    let sender = get_random_address(&network.get_addresses(), vec![]);
    let coin1 = get_random_iota(&client, sender, vec![]).await;
    let coin2 = get_random_iota(&client, sender, vec![coin1.0]).await;
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
    let tx = client
        .transaction_builder()
        .request_add_stake(
            sender,
            vec![coin1.0, coin2.0],
            None,
            validator,
            None,
            rgp * TEST_ONLY_GAS_UNIT_FOR_STAKING,
        )
        .await
        .unwrap();
    let pt = match tx.into_kind() {
        TransactionKind::ProgrammableTransaction(pt) => pt,
        _ => unreachable!(),
    };
    test_transaction(
        &client,
        keystore,
        vec![],
        sender,
        pt,
        vec![],
        rgp * TEST_ONLY_GAS_UNIT_FOR_STAKING,
        rgp,
        false,
    )
    .await;
}

#[tokio::test]
async fn test_pay_all_iota() {
    let network = TestClusterBuilder::new().build().await;
    let client = network.wallet.get_client().await.unwrap();
    let keystore = network.wallet.config().keystore();
    let rgp = network.get_reference_gas_price().await;

    // Test Pay All IOTA
    let addresses = network.get_addresses();
    let sender = get_random_address(&addresses, vec![]);
    let recipient = get_random_address(&addresses, vec![sender]);
    let coin1 = get_random_iota(&client, sender, vec![]).await;
    let coin2 = get_random_iota(&client, sender, vec![coin1.0]).await;
    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();
        builder.pay_all_iota(recipient);
        builder.finish()
    };
    test_transaction(
        &client,
        keystore,
        vec![recipient],
        sender,
        pt,
        vec![coin1, coin2],
        rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
        rgp,
        false,
    )
    .await;
}

#[tokio::test]
async fn test_delegation_parsing() -> Result<(), anyhow::Error> {
    let network = TestClusterBuilder::new().build().await;
    let rgp = network.get_reference_gas_price().await;
    let client = network.wallet.get_client().await.unwrap();
    let sender = get_random_address(&network.get_addresses(), vec![]);
    let gas = get_random_iota(&client, sender, vec![]).await;
    let system_state = client
        .governance_api()
        .get_latest_iota_system_state()
        .await
        .unwrap();
    let active_validators = match system_state {
        IotaSystemStateSummary::V1(v1) => v1.active_validators,
        IotaSystemStateSummary::V2(v2) => v2.active_validators,
        _ => anyhow::bail!("unsupported IotaSystemStateSummary"),
    };
    let validator = active_validators[0].iota_address;

    let ops: Operations = serde_json::from_value(json!(
        [{
            "operation_identifier":{"index":0},
            "type":"Stake",
            "account": { "address" : sender.to_string() },
            "amount" : { "value": "-100000" , "currency": { "symbol": "IOTA", "decimals": 9}},
            "metadata": { "Stake" : {"validator": validator.to_string()} }
        }]
    ))
    .unwrap();
    let metadata = ConstructionMetadata {
        sender,
        coins: vec![gas],
        objects: vec![],
        total_coin_value: 0,
        gas_price: rgp,
        budget: rgp * TEST_ONLY_GAS_UNIT_FOR_STAKING,
    };
    let parsed_data = ops.clone().into_internal()?.try_into_data(metadata)?;
    assert_eq!(ops, Operations::from_transaction_data(parsed_data, None)?);

    Ok(())
}

fn find_module_object(
    changes: &[ObjectChange],
    type_pred: impl Fn(&StructTag) -> bool,
) -> OwnedObjectRef {
    let mut results: Vec<_> = changes
        .iter()
        .filter_map(|change| {
            if let ObjectChange::Created {
                object_id,
                object_type,
                owner,
                version,
                digest,
                ..
            } = change
            {
                if type_pred(object_type) {
                    return Some(OwnedObjectRef {
                        owner: *owner,
                        reference: IotaObjectRef {
                            object_id: *object_id,
                            version: *version,
                            digest: *digest,
                        },
                    });
                }
            };
            None
        })
        .collect();
    // Check that there is only one object found, and hence no ambiguity.
    assert_eq!(results.len(), 1);
    results.pop().unwrap()
}

// Record current IOTA balance of an address then execute the transaction,
// and compare the balance change reported by the event against the actual
// balance change.
async fn test_transaction(
    client: &IotaClient,
    keystore: &Keystore,
    addr_to_check: Vec<IotaAddress>,
    sender: IotaAddress,
    tx: ProgrammableTransaction,
    gas: Vec<ObjectRef>,
    gas_budget: u64,
    gas_price: u64,
    expect_fail: bool,
) -> IotaTransactionBlockResponse {
    let gas = if !gas.is_empty() {
        gas
    } else {
        let input_objects = tx
            .input_objects()
            .unwrap_or_default()
            .iter()
            .flat_map(|obj| {
                if let InputObjectKind::ImmOrOwnedMoveObject((id, ..)) = obj {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        vec![get_random_iota(client, sender, input_objects).await]
    };

    let data = TransactionData::new_with_gas_coins(
        TransactionKind::programmable(tx.clone()),
        sender,
        gas,
        gas_budget,
        gas_price,
    );

    let signature = keystore
        .sign_secure(&data.sender(), &data, Intent::iota_transaction())
        .unwrap();

    // Balance before execution
    let mut balances = BTreeMap::new();
    let mut addr_to_check = addr_to_check;
    addr_to_check.push(sender);
    for addr in addr_to_check {
        balances.insert(addr, get_balance(client, addr).await);
    }

    let response = client
        .quorum_driver_api()
        .execute_transaction_block(
            Transaction::from_data(data.clone(), vec![signature]),
            IotaTransactionBlockResponseOptions::full_content(),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await
        .map_err(|e| anyhow!("TX execution failed for {data:#?}, error : {e}"))
        .unwrap();

    let effects = response.effects.as_ref().unwrap();

    if !expect_fail {
        assert_eq!(
            IotaExecutionStatus::Success,
            *effects.status(),
            "TX execution failed for {:#?}",
            data
        );
    } else {
        assert!(matches!(
            effects.status(),
            IotaExecutionStatus::Failure { .. }
        ));
    }

    let ops = response.clone().try_into().unwrap();
    let balances_from_ops = extract_balance_changes_from_ops(ops);

    // get actual balance changed after transaction
    let mut actual_balance_change = HashMap::new();
    for (addr, balance) in balances {
        let new_balance = get_balance(client, addr).await as i128;
        let balance_changed = new_balance - balance as i128;
        actual_balance_change.insert(addr, balance_changed);
    }
    assert_eq!(
        actual_balance_change, balances_from_ops,
        "balance check failed for tx: {}\neffect:{:#?}",
        tx, effects
    );
    response
}

fn extract_balance_changes_from_ops(ops: Operations) -> HashMap<IotaAddress, i128> {
    ops.into_iter()
        .fold(HashMap::<IotaAddress, i128>::new(), |mut changes, op| {
            if let Some(OperationStatus::Success) = op.status {
                match op.type_ {
                    OperationType::IotaBalanceChange
                    | OperationType::Gas
                    | OperationType::PayIota
                    | OperationType::StakeReward
                    | OperationType::StakePrinciple
                    | OperationType::Stake => {
                        if let (Some(addr), Some(amount)) = (op.account, op.amount) {
                            *changes.entry(addr.address).or_default() += amount.value
                        }
                    }
                    _ => {}
                };
            }
            changes
        })
}

async fn get_random_iota(
    client: &IotaClient,
    sender: IotaAddress,
    except: Vec<ObjectID>,
) -> ObjectRef {
    let coins = client
        .read_api()
        .get_owned_objects(
            sender,
            Some(IotaObjectResponseQuery::new_with_options(
                IotaObjectDataOptions::new()
                    .with_type()
                    .with_owner()
                    .with_previous_transaction(),
            )),
            // cursor
            None,
            // limit
            None,
        )
        .await
        .unwrap()
        .data;

    let coin_resp = coins
        .iter()
        .filter(|object| {
            let obj = object.object().unwrap();
            obj.is_gas_coin() && !except.contains(&obj.object_id)
        })
        .choose(&mut OsRng)
        .unwrap();

    let coin = coin_resp.object().unwrap();
    (coin.object_id, coin.version, coin.digest)
}

fn get_random_address(addresses: &[IotaAddress], except: Vec<IotaAddress>) -> IotaAddress {
    *addresses
        .iter()
        .filter(|addr| !except.contains(*addr))
        .choose(&mut OsRng)
        .unwrap()
}

async fn get_balance(client: &IotaClient, address: IotaAddress) -> u64 {
    let coins = client
        .read_api()
        .get_owned_objects(
            address,
            Some(IotaObjectResponseQuery::new_with_options(
                IotaObjectDataOptions::new()
                    .with_type()
                    .with_owner()
                    .with_previous_transaction(),
            )),
            // cursor
            None,
            // limit
            None,
        )
        .await
        .unwrap()
        .data;

    let mut balance = 0u64;
    for coin in coins {
        let obj = coin.into_object().unwrap();
        if obj.is_gas_coin() {
            let object = client
                .read_api()
                .get_object_with_options(obj.object_id, IotaObjectDataOptions::new().with_bcs())
                .await
                .unwrap();
            let coin: GasCoin = object
                .into_object()
                .unwrap()
                .bcs
                .unwrap()
                .try_as_move()
                .unwrap()
                .deserialize()
                .unwrap();
            balance += coin.value()
        }
    }
    balance
}
