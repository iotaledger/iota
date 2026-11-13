// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeSet;

use iota_json_rpc_api::{
    GovernanceReadApiClient, IndexerApiClient, QUERY_MAX_RESULT_LIMIT, TransactionBuilderClient,
};
use iota_json_rpc_types::{
    DelegatedStake, DelegatedTimelockedStake, IotaObjectDataOptions, IotaObjectResponseQuery,
    ObjectsPage, StakeStatus, TransactionBlockBytes,
};
use iota_protocol_config::{ProtocolConfig, ProtocolVersion};
use iota_sdk_move_types::iota_framework::object::UID;
use iota_sdk_types::{
    Address, Identifier, MoveStruct, ObjectData, ObjectId, Owner, StructTag, TransactionDigest,
    TypeTag,
};
use iota_swarm_config::genesis_config::{
    AccountConfig, ValidatorGenesisConfig, ValidatorGenesisConfigBuilder,
};
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::{
    crypto::{AccountPrivateKey, deterministic_random_account_private_key, get_key_pair},
    iota_system_state::iota_system_state_summary::IotaSystemStateSummary,
    object::{MoveStructExt, OBJECT_START_VERSION, ObjectInner},
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    timelock::{
        label::label_struct_tag_to_string, stardust_upgrade_label::stardust_upgrade_label_type,
        timelock::TimeLock,
    },
    transaction::CallArg,
    utils::to_sender_signed_transaction,
};
use jsonrpsee::http_client::HttpClient;
use rand::{rand_core::UnwrapErr, rngs::SysRng};
use test_cluster::TestCluster;

use crate::common::{
    ApiTestSetup, indexer_wait_for_checkpoint, indexer_wait_for_latest_checkpoint,
    indexer_wait_for_object, indexer_wait_for_transaction, input_size_limit_exceeded_msg,
    rpc_call_error_msg_matches, start_test_cluster_with_read_write_indexer,
};

#[tokio::test]
async fn test_staking() {
    let (cluster, store, client) =
        &start_test_cluster_with_read_write_indexer(Some("test_staking"), None, None).await;

    indexer_wait_for_checkpoint(store, 1).await;

    let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();

    let gas = cluster
        .fund_address_and_return_gas(
            cluster.get_reference_gas_price().await,
            Some(10_000_000_000),
            sender,
        )
        .await;

    indexer_wait_for_object(client, gas.object_id, gas.version).await;

    let iota_coin_ref = cluster
        .fund_address_and_return_gas(
            cluster.get_reference_gas_price().await,
            Some(10_000_000_000),
            sender,
        )
        .await;

    indexer_wait_for_object(client, iota_coin_ref.object_id, iota_coin_ref.version).await;

    // Check StakedIota object before test
    let staked_iota: Vec<DelegatedStake> = client.get_stakes(sender).await.unwrap();
    assert!(staked_iota.is_empty());

    let iota_system_state = client
        .get_latest_iota_system_state_v2()
        .await
        .unwrap()
        .into();
    let validator = match iota_system_state {
        IotaSystemStateSummary::V1(v1) => v1.active_validators[0].iota_address,
        IotaSystemStateSummary::V2(v2) => v2.active_validators[0].iota_address,
        _ => unimplemented!(
            "a new IotaSystemStateSummary enum variant was added and needs to be handled"
        ),
    };

    // Delegate some IOTA
    let transaction_bytes: TransactionBlockBytes = client
        .request_add_stake(
            sender,
            vec![iota_coin_ref.object_id],
            Some(1000000000.into()),
            validator,
            Some(gas.object_id),
            100_000_000.into(),
        )
        .await
        .unwrap();

    let txn = to_sender_signed_transaction(transaction_bytes.to_data().unwrap(), &sender_key);

    let res = cluster.wallet.execute_transaction_must_succeed(txn).await;
    indexer_wait_for_transaction(res.digest, store, client).await;

    let staked_iota: Vec<DelegatedStake> = client.get_stakes(sender).await.unwrap();
    assert_eq!(1, staked_iota.len());
    assert_eq!(1000000000, staked_iota[0].stakes[0].principal);
    assert!(matches!(
        staked_iota[0].stakes[0].status,
        StakeStatus::Pending
    ));
    let staked_iota_by_id = client
        .get_stakes_by_ids(vec![staked_iota[0].stakes[0].staked_iota_id])
        .await
        .unwrap();
    assert_eq!(
        staked_iota[0].stakes[0].staked_iota_id,
        staked_iota_by_id[0].stakes[0].staked_iota_id
    );

    cluster.force_new_epoch().await;
    indexer_wait_for_latest_checkpoint(store, cluster).await;

    // Check DelegatedStake object after epoch transition
    let staked_iota: Vec<DelegatedStake> = client.get_stakes(sender).await.unwrap();
    assert_eq!(1, staked_iota.len());
    assert_eq!(1000000000, staked_iota[0].stakes[0].principal);
    assert!(matches!(
        staked_iota[0].stakes[0].status,
        StakeStatus::Active {
            estimated_reward: _
        }
    ));
}

#[tokio::test]
async fn test_unstaking() {
    let (cluster, store, client) =
        &start_test_cluster_with_read_write_indexer(Some("test_unstaking"), None, None).await;

    indexer_wait_for_checkpoint(store, 1).await;

    let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();

    let gas = cluster
        .fund_address_and_return_gas(
            cluster.get_reference_gas_price().await,
            Some(10_000_000_000),
            sender,
        )
        .await;

    indexer_wait_for_object(client, gas.object_id, gas.version).await;

    let iota_coin_ref = cluster
        .fund_address_and_return_gas(
            cluster.get_reference_gas_price().await,
            Some(10_000_000_000),
            sender,
        )
        .await;

    indexer_wait_for_object(client, iota_coin_ref.object_id, iota_coin_ref.version).await;

    // Check StakedIota object before test
    let staked_iota: Vec<DelegatedStake> = client.get_stakes(sender).await.unwrap();
    assert!(staked_iota.is_empty());

    let iota_system_state = client
        .get_latest_iota_system_state_v2()
        .await
        .unwrap()
        .into();
    let validator = match iota_system_state {
        IotaSystemStateSummary::V1(v1) => v1.active_validators[0].iota_address,
        IotaSystemStateSummary::V2(v2) => v2.active_validators[0].iota_address,
        _ => unimplemented!(
            "a new IotaSystemStateSummary enum variant was added and needs to be handled"
        ),
    };

    // Delegate some IOTA
    let transaction_bytes: TransactionBlockBytes = client
        .request_add_stake(
            sender,
            vec![iota_coin_ref.object_id],
            Some(1000000000.into()),
            validator,
            Some(gas.object_id),
            100_000_000.into(),
        )
        .await
        .unwrap();

    let txn = to_sender_signed_transaction(transaction_bytes.to_data().unwrap(), &sender_key);

    let res = cluster.wallet.execute_transaction_must_succeed(txn).await;
    indexer_wait_for_transaction(res.digest, store, client).await;

    cluster.force_new_epoch().await;
    indexer_wait_for_latest_checkpoint(store, cluster).await;

    // Check DelegatedStake object
    let staked_iota: Vec<DelegatedStake> = client.get_stakes(sender).await.unwrap();
    assert_eq!(1, staked_iota.len());
    assert_eq!(1000000000, staked_iota[0].stakes[0].principal);
    assert!(matches!(
        staked_iota[0].stakes[0].status,
        StakeStatus::Active {
            estimated_reward: _
        }
    ));

    let transaction_bytes: TransactionBlockBytes = client
        .request_withdraw_stake(
            sender,
            staked_iota[0].stakes[0].staked_iota_id,
            Some(gas.object_id),
            100_000_000.into(),
        )
        .await
        .unwrap();

    let txn = to_sender_signed_transaction(transaction_bytes.to_data().unwrap(), &sender_key);

    let res = cluster.wallet.execute_transaction_must_succeed(txn).await;
    indexer_wait_for_transaction(res.digest, store, client).await;

    cluster.force_new_epoch().await;
    indexer_wait_for_latest_checkpoint(store, cluster).await;

    let indexer_response = client
        .get_stakes_by_ids(vec![staked_iota[0].stakes[0].staked_iota_id])
        .await
        .unwrap();
    assert_eq!(0, indexer_response.len());

    let staked_iota: Vec<DelegatedStake> = client.get_stakes(sender).await.unwrap();
    assert!(staked_iota.is_empty());
}

#[tokio::test]
async fn test_timelocked_staking() {
    let (cluster, store, client) =
        &start_test_cluster_with_read_write_indexer(Some("test_timelocked_staking"), None, None)
            .await;

    indexer_wait_for_checkpoint(store, 1).await;

    let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();

    let gas = cluster
        .fund_address_and_return_gas(
            cluster.get_reference_gas_price().await,
            Some(10_000_000_000),
            sender,
        )
        .await;

    indexer_wait_for_object(client, gas.object_id, gas.version).await;

    let iota_coin_ref = cluster
        .fund_address_and_return_gas(
            cluster.get_reference_gas_price().await,
            Some(10_000_000_000),
            sender,
        )
        .await;

    indexer_wait_for_object(client, iota_coin_ref.object_id, iota_coin_ref.version).await;

    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();

        let iota_coin_argument = builder
            .input(CallArg::ImmutableOrOwned(iota_coin_ref))
            .expect("valid obj");

        // Step 1: Get the IOTA balance from the coin object.
        let iota_balance = builder.programmable_move_call(
            ObjectId::FRAMEWORK,
            Identifier::COIN_MODULE,
            Identifier::from_static("into_balance"),
            vec![TypeTag::from(StructTag::new_gas())],
            vec![iota_coin_argument],
        );

        // Step 2: Timelock the IOTA balance.
        let timelock_timestamp = builder.pure(u64::MAX).unwrap();
        let timelocked_iota_balance = builder.programmable_move_call(
            ObjectId::FRAMEWORK,
            Identifier::from_static("timelock"),
            Identifier::from_static("lock"),
            vec![TypeTag::Struct(Box::new(StructTag::new_balance(
                TypeTag::from(StructTag::new_gas()),
            )))],
            vec![iota_balance, timelock_timestamp],
        );

        // Step 3: Delegate the timelocked IOTA balance.
        let iota_system_state = client
            .get_latest_iota_system_state_v2()
            .await
            .unwrap()
            .into();
        let validator = match iota_system_state {
            IotaSystemStateSummary::V1(v1) => v1.active_validators[0].iota_address,
            IotaSystemStateSummary::V2(v2) => v2.active_validators[0].iota_address,
            _ => unimplemented!(
                "a new IotaSystemStateSummary enum variant was added and needs to be handled"
            ),
        };

        let validator = builder.pure(validator).unwrap();
        let state = builder.input(CallArg::IOTA_SYSTEM_MUTABLE).unwrap();

        let _ = builder.programmable_move_call(
            ObjectId::SYSTEM,
            Identifier::TIMELOCKED_STAKING_MODULE,
            Identifier::from_static("request_add_stake"),
            vec![],
            vec![state, timelocked_iota_balance, validator],
        );

        builder.finish()
    };

    let context = &cluster.wallet;
    let gas_price = context.get_reference_gas_price().await.unwrap();

    let tx_builder = TestTransactionBuilder::new(sender, gas, gas_price);
    let txn = to_sender_signed_transaction(tx_builder.programmable(pt).build(), &sender_key);

    let res = context.execute_transaction_must_succeed(txn).await;
    indexer_wait_for_transaction(res.digest, store, client).await;

    cluster.force_new_epoch().await;
    indexer_wait_for_latest_checkpoint(store, cluster).await;

    let staked_iota: Vec<DelegatedTimelockedStake> =
        client.get_timelocked_stakes(sender).await.unwrap();

    assert_eq!(staked_iota.len(), 1);
    assert_eq!(10000000000, staked_iota[0].stakes[0].principal);
    assert!(matches!(
        staked_iota[0].stakes[0].status,
        StakeStatus::Active {
            estimated_reward: _
        }
    ));
}

#[tokio::test]
async fn test_timelocked_unstaking() {
    let (cluster, store, client) =
        &start_test_cluster_with_read_write_indexer(Some("test_timelocked_unstaking"), None, None)
            .await;

    indexer_wait_for_checkpoint(store, 1).await;

    let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();

    let gas = cluster
        .fund_address_and_return_gas(
            cluster.get_reference_gas_price().await,
            Some(10_000_000_000),
            sender,
        )
        .await;

    indexer_wait_for_object(client, gas.object_id, gas.version).await;

    let iota_coin_ref = cluster
        .fund_address_and_return_gas(
            cluster.get_reference_gas_price().await,
            Some(10_000_000_000),
            sender,
        )
        .await;

    indexer_wait_for_object(client, iota_coin_ref.object_id, iota_coin_ref.version).await;

    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();

        let iota_coin_argument = builder
            .input(CallArg::ImmutableOrOwned(iota_coin_ref))
            .expect("valid obj");

        // Step 1: Get the IOTA balance from the coin object.
        let iota_balance = builder.programmable_move_call(
            ObjectId::FRAMEWORK,
            Identifier::COIN_MODULE,
            Identifier::from_static("into_balance"),
            vec![TypeTag::from(StructTag::new_gas())],
            vec![iota_coin_argument],
        );

        // Step 2: Timelock the IOTA balance.
        let timelock_timestamp = builder.pure(u64::MAX).unwrap();
        let timelocked_iota_balance = builder.programmable_move_call(
            ObjectId::FRAMEWORK,
            Identifier::from_static("timelock"),
            Identifier::from_static("lock"),
            vec![TypeTag::Struct(Box::new(StructTag::new_balance(
                TypeTag::from(StructTag::new_gas()),
            )))],
            vec![iota_balance, timelock_timestamp],
        );

        // Step 3: Delegate the timelocked IOTA balance.
        let iota_system_state = client
            .get_latest_iota_system_state_v2()
            .await
            .unwrap()
            .into();
        let validator = match iota_system_state {
            IotaSystemStateSummary::V1(v1) => v1.active_validators[0].iota_address,
            IotaSystemStateSummary::V2(v2) => v2.active_validators[0].iota_address,
            _ => unimplemented!(
                "a new IotaSystemStateSummary enum variant was added and needs to be handled"
            ),
        };

        let validator = builder.pure(validator).unwrap();
        let state = builder.input(CallArg::IOTA_SYSTEM_MUTABLE).unwrap();

        let _ = builder.programmable_move_call(
            ObjectId::SYSTEM,
            Identifier::TIMELOCKED_STAKING_MODULE,
            Identifier::from_static("request_add_stake"),
            vec![],
            vec![state, timelocked_iota_balance, validator],
        );

        builder.finish()
    };

    let context = &cluster.wallet;
    let gas_price = context.get_reference_gas_price().await.unwrap();

    let tx_builder = TestTransactionBuilder::new(sender, gas, gas_price);
    let txn = to_sender_signed_transaction(tx_builder.programmable(pt).build(), &sender_key);

    let res = context.execute_transaction_must_succeed(txn).await;
    indexer_wait_for_transaction(res.digest, store, client).await;

    cluster.force_new_epoch().await;
    indexer_wait_for_latest_checkpoint(store, cluster).await;

    let staked_iota = client.get_timelocked_stakes(sender).await.unwrap();

    assert_eq!(staked_iota.len(), 1);
    assert_eq!(10000000000, staked_iota[0].stakes[0].principal);
    assert!(matches!(
        staked_iota[0].stakes[0].status,
        StakeStatus::Active {
            estimated_reward: _
        }
    ));

    let timelocked_stake_id = staked_iota[0].stakes[0].timelocked_staked_iota_id;
    let timelocked_stake_id_ref = cluster
        .wallet
        .get_object_ref(timelocked_stake_id)
        .await
        .unwrap();

    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();

        let timelocked_stake_id_argument = builder
            .input(CallArg::ImmutableOrOwned(timelocked_stake_id_ref))
            .expect("valid obj");

        let state = builder.input(CallArg::IOTA_SYSTEM_MUTABLE).unwrap();

        let _ = builder.programmable_move_call(
            ObjectId::SYSTEM,
            Identifier::TIMELOCKED_STAKING_MODULE,
            Identifier::from_static("request_withdraw_stake"),
            vec![],
            vec![state, timelocked_stake_id_argument],
        );

        builder.finish()
    };

    let gas = cluster.wallet.get_object_ref(gas.object_id).await.unwrap();
    let tx_builder = TestTransactionBuilder::new(sender, gas, gas_price);
    let txn = to_sender_signed_transaction(tx_builder.programmable(pt).build(), &sender_key);

    let res = context.execute_transaction_must_succeed(txn).await;
    indexer_wait_for_transaction(res.digest, store, client).await;

    cluster.force_new_epoch().await;
    indexer_wait_for_latest_checkpoint(store, cluster).await;

    let res = client.get_timelocked_stakes(sender).await.unwrap();
    assert_eq!(res.len(), 0);

    let res = client
        .get_timelocked_stakes_by_ids(vec![timelocked_stake_id])
        .await
        .unwrap();

    assert_eq!(res.len(), 0);
}

#[tokio::test]
async fn get_latest_iota_system_state_v2() {
    // We use dedicated cluster since we are changing epochs
    let (cluster, store, client) = &start_test_cluster_with_read_write_indexer(
        Some("governance_get_latest_iota_system_state_v2"),
        None,
        None,
    )
    .await;

    // ensure that the system state is updated
    cluster.force_new_epoch().await;
    // when epoch is advanced, the indexer should catch up the latest checkpoint to
    // reflect the end of epoch checkpoint changes
    indexer_wait_for_latest_checkpoint(store, cluster).await;

    let system_state = client
        .get_latest_iota_system_state_v2()
        .await
        .unwrap()
        .into();

    let IotaSystemStateSummary::V2(system_state_v2) = system_state else {
        panic!("expected IotaSystemStateSummaryV2");
    };
    assert_eq!(
        system_state_v2.protocol_version,
        ProtocolVersion::MAX.as_u64()
    );
    assert_eq!(system_state_v2.system_state_version, 2);
}

#[test]
fn get_latest_iota_system_state_reports_protocol_config_validator_parameters() {
    let ApiTestSetup {
        runtime,
        cluster,
        store,
        client,
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        indexer_wait_for_checkpoint(store, 1).await;

        let config = cluster.protocol_config();
        let summary: IotaSystemStateSummary = client
            .get_latest_iota_system_state_v2()
            .await
            .unwrap()
            .into();
        assert_eq!(summary.min_validator_count(), config.min_validator_count());
        assert_eq!(summary.max_validator_count(), config.max_validator_count());
        assert_eq!(
            summary.min_validator_joining_stake(),
            config.min_validator_joining_stake()
        );
        assert_eq!(
            summary.validator_low_stake_threshold(),
            config.validator_low_stake_threshold()
        );
        assert_eq!(
            summary.validator_very_low_stake_threshold(),
            config.validator_very_low_stake_threshold()
        );
        assert_eq!(
            summary.validator_low_stake_grace_period(),
            config.validator_low_stake_grace_period()
        );
    });
}

#[tokio::test]
async fn get_timelocked_stakes_reports_the_stake_details() {
    let (address, key) = deterministic_random_account_private_key();

    let principal = 100_000_000_000;
    let expiration_timestamp_ms = u64::MAX;
    let label = Some(label_struct_tag_to_string(stardust_upgrade_label_type()));

    let timelock_iota = MoveStruct::new_from_execution(
        StructTag::new_timelocked_gas_balance(),
        OBJECT_START_VERSION,
        TimeLock::<iota_types::balance::Balance>::new(
            UID::new(ObjectId::random()),
            iota_types::balance::Balance::new(principal),
            expiration_timestamp_ms,
            label.clone(),
        )
        .to_bcs_bytes(),
        &ProtocolConfig::get_for_min_version(),
        // system_mutation
        false,
    )
    .unwrap();
    let timelock_iota = ObjectInner {
        owner: Owner::Address(address),
        data: ObjectData::Struct(timelock_iota),
        previous_transaction: TransactionDigest::GENESIS_MARKER,
        storage_rebate: 0,
    };

    let (cluster, store, client) = &start_test_cluster_with_read_write_indexer(
        Some("governance_get_timelocked_stakes_reports_the_stake_details"),
        Some(Box::new(move |builder| {
            builder
                .with_accounts(
                    [AccountConfig {
                        address: Some(address),
                        gas_amounts: [100_000_000].into(),
                    }]
                    .into(),
                )
                .with_objects([timelock_iota.into()])
        })),
        None,
    )
    .await;
    indexer_wait_for_checkpoint(store, 1).await;

    let objects: ObjectsPage = client
        .get_owned_objects(
            address,
            Some(IotaObjectResponseQuery::new_with_options(
                IotaObjectDataOptions::new().with_type(),
            )),
            None,
            None,
        )
        .await
        .unwrap();
    assert_eq!(2, objects.data.len());
    let (coin, timelock): (Vec<_>, Vec<_>) = objects
        .data
        .into_iter()
        .partition(|o| o.data.as_ref().unwrap().is_gas_coin());
    let coin = coin[0].object().unwrap().object_id;
    let timelocked_balance = timelock[0].object().unwrap().object_id;

    assert!(
        client
            .get_timelocked_stakes(address)
            .await
            .unwrap()
            .is_empty()
    );

    let validator = first_active_validator(client).await;

    let transaction_bytes: TransactionBlockBytes = client
        .request_add_timelocked_stake(
            address,
            timelocked_balance,
            validator,
            coin,
            100_000_000.into(),
        )
        .await
        .unwrap();
    let txn = to_sender_signed_transaction(transaction_bytes.to_data().unwrap(), &key);
    let res = cluster.wallet.execute_transaction_must_succeed(txn).await;
    indexer_wait_for_transaction(res.digest, store, client).await;

    let staked_iota: Vec<DelegatedTimelockedStake> =
        client.get_timelocked_stakes(address).await.unwrap();
    assert_eq!(1, staked_iota.len());
    let staked_iota = &staked_iota[0];
    assert_eq!(1, staked_iota.stakes.len());
    let stake = &staked_iota.stakes[0];

    assert_eq!(validator, staked_iota.validator_address);
    assert_eq!(principal, stake.principal);
    assert!(matches!(stake.status, StakeStatus::Pending));
    assert_eq!(expiration_timestamp_ms, stake.expiration_timestamp_ms);
    assert_eq!(label, stake.label);

    let staked_iota_by_id = client
        .get_timelocked_stakes_by_ids(vec![stake.timelocked_staked_iota_id])
        .await
        .unwrap();
    assert_eq!(1, staked_iota_by_id.len());
    let staked_iota_by_id = &staked_iota_by_id[0];
    assert_eq!(1, staked_iota_by_id.stakes.len());
    let stake_by_id = &staked_iota_by_id.stakes[0];

    assert_eq!(
        staked_iota.validator_address,
        staked_iota_by_id.validator_address
    );
    assert_eq!(staked_iota.staking_pool, staked_iota_by_id.staking_pool);
    assert_eq!(
        stake.timelocked_staked_iota_id,
        stake_by_id.timelocked_staked_iota_id
    );
    assert_eq!(stake.stake_request_epoch, stake_by_id.stake_request_epoch);
    assert_eq!(stake.stake_active_epoch, stake_by_id.stake_active_epoch);
    assert_eq!(stake.principal, stake_by_id.principal);
    assert!(matches!(stake_by_id.status, StakeStatus::Pending));
    assert_eq!(
        stake.expiration_timestamp_ms,
        stake_by_id.expiration_timestamp_ms
    );
    assert_eq!(stake.label, stake_by_id.label);
}

/// Registers `new_validator` as a candidate, stakes the minimum joining stake
/// on it and requests to join the validator set, so that it becomes active at
/// the next epoch change.
async fn execute_add_validator_transactions(
    cluster: &TestCluster,
    new_validator: &ValidatorGenesisConfig,
) {
    let address = new_validator.account_key_pair.public_key().derive_address();
    let rgp = cluster.get_reference_gas_price().await;

    let gas = cluster
        .wallet
        .get_one_gas_object_owned_by_address(address)
        .await
        .unwrap()
        .unwrap();
    let tx = TestTransactionBuilder::new(address, gas, rgp)
        .call_request_add_validator_candidate(
            &new_validator.to_validator_info_with_random_name().into(),
        )
        .build_and_sign(&new_validator.account_key_pair);
    cluster.execute_transaction(tx).await;

    let min_validator_joining_stake = cluster.protocol_config().min_validator_joining_stake();
    let stake_coin = cluster
        .wallet
        .gas_for_owner_budget(address, min_validator_joining_stake, Default::default())
        .await
        .unwrap()
        .1
        .object_ref();
    let gas = cluster
        .wallet
        .gas_for_owner_budget(address, 0, BTreeSet::from([stake_coin.object_id]))
        .await
        .unwrap()
        .1
        .object_ref();
    let stake_tx = TestTransactionBuilder::new(address, gas, rgp)
        .call_staking(stake_coin, address)
        .build_and_sign(&new_validator.account_key_pair);
    cluster.execute_transaction(stake_tx).await;

    let gas = cluster.wallet.get_object_ref(gas.object_id).await.unwrap();
    let tx = TestTransactionBuilder::new(address, gas, rgp)
        .call_request_add_validator()
        .build_and_sign(&new_validator.account_key_pair);
    cluster.execute_transaction(tx).await;
}

/// Returns the status of the stake the validator at `address` placed on itself
/// in epoch 0.
async fn initial_self_stake_status(client: &HttpClient, address: Address) -> StakeStatus {
    let stakes = client.get_stakes(address).await.unwrap();
    stakes[0]
        .stakes
        .iter()
        .find(|s| s.stake_request_epoch == 0)
        .unwrap()
        .status
        .clone()
}

#[tokio::test]
async fn get_stakes_with_new_validator() {
    let new_validator = ValidatorGenesisConfigBuilder::new().build(&mut UnwrapErr(SysRng));
    let address = new_validator.account_key_pair.public_key().derive_address();

    let (mut cluster, store, client) = start_test_cluster_with_read_write_indexer(
        Some("governance_get_stakes_with_new_validator"),
        Some(Box::new(move |builder| {
            builder
                .with_validator_candidates([address])
                .with_num_validators(4)
        })),
        None,
    )
    .await;
    indexer_wait_for_checkpoint(&store, 1).await;

    assert!(client.get_stakes(address).await.unwrap().is_empty());
    assert!(client.get_committee_info(Some(1.into())).await.is_err());

    execute_add_validator_transactions(&cluster, &new_validator).await;
    indexer_wait_for_latest_checkpoint(&store, &cluster).await;

    // The validator only becomes active at the next epoch change.
    assert!(matches!(
        initial_self_stake_status(&client, address).await,
        StakeStatus::Pending
    ));

    cluster.force_new_epoch().await;
    cluster.spawn_new_validator(new_validator).await;
    cluster.wait_for_epoch_all_nodes(1).await;
    indexer_wait_for_latest_checkpoint(&store, &cluster).await;

    // An active validator joins the committee only one epoch after activation,
    // so the stake is active but has not earned any reward yet.
    let committee = client.get_committee_info(Some(1.into())).await.unwrap();
    assert_eq!(committee.epoch, 1);
    assert_eq!(committee.validators.len(), 4);
    assert!(matches!(
        initial_self_stake_status(&client, address).await,
        StakeStatus::Active {
            estimated_reward: 0
        }
    ));

    cluster.force_new_epoch().await;
    cluster.wait_for_epoch_all_nodes(2).await;
    indexer_wait_for_latest_checkpoint(&store, &cluster).await;

    let committee = client.get_committee_info(Some(2.into())).await.unwrap();
    assert_eq!(committee.epoch, 2);
    assert_eq!(committee.validators.len(), 5);
    assert!(matches!(
        initial_self_stake_status(&client, address).await,
        StakeStatus::Active {
            estimated_reward: 0
        }
    ));

    cluster.force_new_epoch().await;
    indexer_wait_for_latest_checkpoint(&store, &cluster).await;

    // One epoch in the committee earns the stake a reward.
    assert!(matches!(
        initial_self_stake_status(&client, address).await,
        StakeStatus::Active { estimated_reward } if estimated_reward > 0
    ));
}

async fn first_active_validator(client: &HttpClient) -> Address {
    match client
        .get_latest_iota_system_state_v2()
        .await
        .unwrap()
        .into()
    {
        IotaSystemStateSummary::V1(v1) => v1.active_validators[0].iota_address,
        IotaSystemStateSummary::V2(v2) => v2.active_validators[0].iota_address,
        _ => unimplemented!(
            "a new IotaSystemStateSummary enum variant was added and needs to be handled"
        ),
    }
}

#[test]
fn get_committee_info() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        ..
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        indexer_wait_for_checkpoint(store, 1).await;

        // Test with no specified epoch
        let response = client.get_committee_info(None).await.unwrap();

        assert_eq!(response.validators.len(), 4);

        // Test with specified epoch 0
        let response = client.get_committee_info(Some(0.into())).await.unwrap();

        let (epoch_id, validators) = (response.epoch, response.validators);

        assert!(epoch_id == 0);
        assert_eq!(validators.len(), 4);

        // Test with non-existent epoch
        let response = client.get_committee_info(Some(u64::MAX.into())).await;

        assert!(response.is_err());
    });
}

#[test]
fn get_reference_gas_price() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        ..
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        indexer_wait_for_checkpoint(store, 1).await;

        let response = client.get_reference_gas_price().await.unwrap();
        assert_eq!(response, 1000.into());
    });
}

#[test]
fn get_validators_apy() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        ..
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        indexer_wait_for_checkpoint(store, 1).await;

        let apys = client.get_validators_apy().await.unwrap().apys;

        assert_eq!(apys.len(), 4);
        assert!(apys.iter().any(|apy| apy.apy >= 0.0));
    });
}

#[test]
fn get_stakes_by_ids_at_and_above_limit() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        ..
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        indexer_wait_for_checkpoint(store, 1).await;

        let at_limit = std::iter::repeat_with(ObjectId::random)
            .take(*QUERY_MAX_RESULT_LIMIT)
            .collect();

        let stakes = client
            .get_stakes_by_ids(at_limit)
            .await
            .expect("request at the limit should succeed");
        assert!(stakes.is_empty());

        let above_limit = std::iter::repeat_with(ObjectId::random)
            .take(*QUERY_MAX_RESULT_LIMIT + 1)
            .collect();

        let result = client.get_stakes_by_ids(above_limit).await;
        assert!(rpc_call_error_msg_matches(
            result,
            &input_size_limit_exceeded_msg()
        ));
    });
}

#[test]
fn get_timelocked_stakes_by_ids_at_and_above_limit() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        ..
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        indexer_wait_for_checkpoint(store, 1).await;

        let at_limit = std::iter::repeat_with(ObjectId::random)
            .take(*QUERY_MAX_RESULT_LIMIT)
            .collect();

        let stakes = client
            .get_timelocked_stakes_by_ids(at_limit)
            .await
            .expect("request at the limit should succeed");
        assert!(stakes.is_empty());

        let above_limit = std::iter::repeat_with(ObjectId::random)
            .take(*QUERY_MAX_RESULT_LIMIT + 1)
            .collect();

        let result = client.get_timelocked_stakes_by_ids(above_limit).await;
        assert!(rpc_call_error_msg_matches(
            result,
            &input_size_limit_exceeded_msg()
        ));
    });
}
