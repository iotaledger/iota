// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{path::PathBuf, str::FromStr, sync::Arc};

use iota_indexer::store::PgIndexerStore;
use iota_json::{IotaJsonValue, call_args, type_args};
use iota_json_rpc_api::{
    CoinReadApiClient, GovernanceReadApiClient, IndexerApiClient, TransactionBuilderClient,
    WriteApiClient,
};
use iota_json_rpc_types::{
    IotaCoinMetadata, IotaObjectData, IotaObjectDataFilter, IotaObjectRef, IotaObjectResponseQuery,
    IotaTransactionBlockEffectsAPI, IotaTransactionBlockResponse,
    IotaTransactionBlockResponseOptions, IotaTypeTag, ObjectChange, TransactionBlockBytes,
};
use iota_move_build::BuildConfig;
use iota_types::{
    IOTA_FRAMEWORK_ADDRESS, TypeTag,
    balance::Supply,
    base_types::{IotaAddress, ObjectID},
    coin::{COIN_MODULE_NAME, CoinMetadata, TreasuryCap},
    crypto::IotaKeyPair,
    iota_system_state::iota_system_state_summary::IotaSystemStateSummary,
    parse_iota_struct_tag,
    quorum_driver_types::ExecuteTransactionRequestType,
    utils::to_sender_signed_transaction,
};
use jsonrpsee::http_client::HttpClient;
use move_core_types::{identifier::Identifier, language_storage::StructTag};
use test_cluster::TestCluster;
use tokio::sync::{Mutex, OnceCell};

use crate::common::{indexer_wait_for_object, indexer_wait_for_transaction};

static PACKAGE_PUBLISH_LOCK: OnceCell<Arc<Mutex<i64>>> = OnceCell::const_new();

pub const FUNDED_BALANCE_PER_COIN: u64 = 10_000_000_000;

/// Fetches the total supply of a coin type from both fullnode and indexer.
pub async fn get_total_supply_fullnode_indexer(
    cluster: &TestCluster,
    client: &HttpClient,
    coin_type: String,
) -> (Option<Supply>, Option<Supply>) {
    let result_fullnode = cluster
        .rpc_client()
        .get_total_supply(coin_type.clone())
        .await
        .ok();
    let result_indexer = client.get_total_supply(coin_type).await.ok();
    (result_fullnode, result_indexer)
}

/// Fetches a single owned object by its type.
pub async fn get_single_owned_object_by_type(
    http_client: &HttpClient,
    address: IotaAddress,
    struct_tag: StructTag,
) -> IotaObjectData {
    http_client
        .get_owned_objects(
            address,
            Some(IotaObjectResponseQuery::new(
                Some(IotaObjectDataFilter::StructType(struct_tag)),
                None,
            )),
            None,
            None,
        )
        .await
        .unwrap()
        .data
        .pop()
        .unwrap()
        .data
        .unwrap()
}

/// Publishes a test Move package and returns the package ID and transaction
/// response.
pub async fn publish_test_move_package(
    client: &HttpClient,
    address: IotaAddress,
    account_keypair: &IotaKeyPair,
    test_package_name: &str,
) -> Result<(ObjectID, IotaTransactionBlockResponse), anyhow::Error> {
    let _lock = PACKAGE_PUBLISH_LOCK
        .get_or_init(async || Arc::new(tokio::sync::Mutex::new(0)))
        .await
        .lock()
        .await;

    let coins = client
        .get_coins(address, None, None, Some(1))
        .await
        .unwrap()
        .data;
    let gas = &coins[0];

    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.extend(["tests", "data", test_package_name]);

    let compiled_package = BuildConfig::default().build(&path).unwrap();
    let with_unpublished_deps = false;
    let compiled_modules_bytes = compiled_package.get_package_base64(with_unpublished_deps);
    let dependencies = compiled_package.get_dependency_storage_package_ids();

    let transaction_bytes: TransactionBlockBytes = client
        .publish(
            address,
            compiled_modules_bytes,
            dependencies,
            Some(gas.coin_object_id),
            100_000_000.into(),
        )
        .await
        .unwrap();

    let signed_transaction =
        to_sender_signed_transaction(transaction_bytes.to_data().unwrap(), account_keypair);
    let (tx_bytes, signatures) = signed_transaction.to_tx_bytes_and_signatures();

    let tx_response: IotaTransactionBlockResponse = client
        .execute_transaction_block(
            tx_bytes,
            signatures,
            Some(
                IotaTransactionBlockResponseOptions::new()
                    .with_object_changes()
                    .with_events(),
            ),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await
        .unwrap();

    let object_changes = tx_response.object_changes.as_ref().unwrap();
    let package_id = object_changes
        .iter()
        .find_map(|change| match change {
            ObjectChange::Published { package_id, .. } => Some(package_id),
            _ => None,
        })
        .unwrap();

    Ok((*package_id, tx_response))
}

/// Creates a migrated coin manager coins for a given address and account
/// keypair.
pub async fn create_migrated_coin_manager_coins(
    cluster: &TestCluster,
    indexer_client: &HttpClient,
    pg_store: &PgIndexerStore,
    address: IotaAddress,
    account_keypair: &IotaKeyPair,
) -> Result<(String, String), anyhow::Error> {
    let (coin_name, immutable_metadata_coin_name) =
        create_trusted_coins(cluster, address, account_keypair).await?;
    mint_trusted_coin(
        cluster,
        coin_name.clone(),
        address,
        account_keypair,
        100_000,
    )
    .await
    .unwrap();

    let http_client = cluster.rpc_client();
    let (package_id, _) = publish_test_move_package(
        http_client,
        address,
        account_keypair,
        "migrate_to_coin_manager",
    )
    .await?;

    {
        let coin_type = parse_iota_struct_tag(&coin_name).unwrap();
        let treasury_cap_type = TreasuryCap::type_(coin_type.clone());
        let treasury_cap = get_single_owned_object_by_type(http_client, address, treasury_cap_type)
            .await
            .object_id;

        let coin_metadata_type = CoinMetadata::type_(coin_type.clone());
        let coin_metadata =
            get_single_owned_object_by_type(http_client, address, coin_metadata_type)
                .await
                .object_id;

        let guardian_type = StructTag {
            address: *package_id,
            module: Identifier::new("coin_manager_coin").unwrap(),
            name: Identifier::new("Guardian").unwrap(),
            type_params: vec![TypeTag::Struct(Box::new(StructTag {
                address: *package_id,
                module: Identifier::new("coin_manager_coin").unwrap(),
                name: Identifier::new("COIN_MANAGER_COIN").unwrap(),
                type_params: vec![],
            }))],
        };
        let guardian = get_single_owned_object_by_type(http_client, address, guardian_type)
            .await
            .object_id;

        let coin_manager_coin_name = format!("{package_id}::coin_manager_coin::COIN_MANAGER_COIN");
        let tx_response = execute_move_call(
            http_client,
            address,
            account_keypair,
            package_id,
            "coin_manager_coin".to_string(),
            "migrate_to_manager".into(),
            type_args![coin_name, coin_manager_coin_name].unwrap(),
            call_args![guardian, treasury_cap, coin_metadata].unwrap(),
        )
        .await?;
        assert_eq!(tx_response.status_ok(), Some(true));
        indexer_wait_for_transaction(tx_response.digest, pg_store, indexer_client).await;
    }

    {
        let imm_coin_type = parse_iota_struct_tag(&immutable_metadata_coin_name).unwrap();
        let treasury_cap_type = TreasuryCap::type_(imm_coin_type.clone());
        let treasury_cap = get_single_owned_object_by_type(http_client, address, treasury_cap_type)
            .await
            .object_id;

        let coin_metadata = http_client
            .get_coin_metadata(immutable_metadata_coin_name.clone())
            .await
            .unwrap()
            .unwrap()
            .id
            .unwrap();

        let guardian_type = StructTag {
            address: *package_id,
            module: Identifier::new("immutable_metadata_coin_manager_coin").unwrap(),
            name: Identifier::new("Guardian").unwrap(),
            type_params: vec![TypeTag::Struct(Box::new(StructTag {
                address: *package_id,
                module: Identifier::new("immutable_metadata_coin_manager_coin").unwrap(),
                name: Identifier::new("IMMUTABLE_METADATA_COIN_MANAGER_COIN").unwrap(),
                type_params: vec![],
            }))],
        };
        let guardian = get_single_owned_object_by_type(http_client, address, guardian_type)
            .await
            .object_id;

        let coin_manager_immutable_metadata_coin_name = format!(
            "{package_id}::immutable_metadata_coin_manager_coin::IMMUTABLE_METADATA_COIN_MANAGER_COIN"
        );
        let tx_response = execute_move_call(
            http_client,
            address,
            account_keypair,
            package_id,
            "immutable_metadata_coin_manager_coin".to_string(),
            "migrate_to_manager".into(),
            type_args![
                immutable_metadata_coin_name,
                coin_manager_immutable_metadata_coin_name
            ]
            .unwrap(),
            call_args![guardian, treasury_cap, coin_metadata].unwrap(),
        )
        .await?;
        assert_eq!(tx_response.status_ok(), Some(true));
        indexer_wait_for_transaction(tx_response.digest, pg_store, indexer_client).await;

        // Hide metadata of immutable coin, so that Node/Indexer can not use it.
        let tx_response = execute_move_call(
            http_client,
            address,
            account_keypair,
            imm_coin_type.address.into(),
            "immutable_metadata_trusted_coin".to_string(),
            "hide_metadata".into(),
            type_args![immutable_metadata_coin_name].unwrap(),
            call_args![coin_metadata].unwrap(),
        )
        .await?;
        assert_eq!(tx_response.status_ok(), Some(true));
        indexer_wait_for_transaction(tx_response.digest, pg_store, indexer_client).await;
    }

    Ok((coin_name, immutable_metadata_coin_name))
}

/// Mints a trusted coin and transfers it to the specified address.
pub async fn mint_trusted_coin(
    cluster: &TestCluster,
    coin_name: String,
    address: IotaAddress,
    account_keypair: &IotaKeyPair,
    amount: u64,
) -> Result<IotaObjectRef, anyhow::Error> {
    let http_client = cluster.rpc_client();

    let result: Supply = http_client
        .get_total_supply(coin_name.clone())
        .await
        .unwrap();
    assert_eq!(0, result.value);

    let coin_type = parse_iota_struct_tag(&coin_name).unwrap();
    let treasury_cap_type = TreasuryCap::type_(coin_type);
    let treasury_cap = get_single_owned_object_by_type(http_client, address, treasury_cap_type)
        .await
        .object_id;

    let tx_response = execute_move_call(
        http_client,
        address,
        account_keypair,
        IOTA_FRAMEWORK_ADDRESS.into(),
        COIN_MODULE_NAME.to_string(),
        "mint_and_transfer".into(),
        type_args![coin_name.clone()].unwrap(),
        call_args![treasury_cap, amount, address].unwrap(),
    )
    .await?;
    assert_eq!(tx_response.status_ok(), Some(true));

    let created_coin_obj_ref = tx_response.effects.unwrap().created()[0].reference.clone();

    Ok(created_coin_obj_ref)
}

/// Executes a move call.
pub async fn execute_move_call(
    client: &HttpClient,
    address: IotaAddress,
    account_keypair: &IotaKeyPair,
    package_object_id: ObjectID,
    module: String,
    function: String,
    type_arguments: Vec<IotaTypeTag>,
    arguments: Vec<IotaJsonValue>,
) -> Result<IotaTransactionBlockResponse, anyhow::Error> {
    let coins = client
        .get_coins(address, None, None, Some(1))
        .await
        .unwrap()
        .data;
    let gas = &coins[0];

    let transaction_bytes: TransactionBlockBytes = client
        .move_call(
            address,
            package_object_id,
            module,
            function,
            type_arguments,
            arguments,
            Some(gas.coin_object_id),
            10_000_000.into(),
            None,
        )
        .await
        .unwrap();

    let signed_transaction =
        to_sender_signed_transaction(transaction_bytes.to_data().unwrap(), account_keypair);
    let (tx_bytes, signatures) = signed_transaction.to_tx_bytes_and_signatures();

    Ok(client
        .execute_transaction_block(
            tx_bytes,
            signatures,
            Some(IotaTransactionBlockResponseOptions::new().with_effects()),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await
        .unwrap())
}

/// Creates a trusted coin and an immutable metadata trusted coin for a given
/// address.
pub async fn create_trusted_coins(
    cluster: &TestCluster,
    address: IotaAddress,
    account_keypair: &IotaKeyPair,
) -> Result<(String, String), anyhow::Error> {
    let http_client = cluster.rpc_client();

    let (package_id, _) = publish_test_move_package(
        http_client,
        address,
        account_keypair,
        "dummy_modules_publish",
    )
    .await?;

    let coin_name = format!("{package_id}::trusted_coin::TRUSTED_COIN");
    let imm_coin_name =
        format!("{package_id}::immutable_metadata_trusted_coin::IMMUTABLE_METADATA_TRUSTED_COIN");

    Ok((coin_name, imm_coin_name))
}

/// Creates a specified number of coins for a given address and waits for the
/// indexer to index them.
pub async fn create_coins_and_wait_for_indexer(
    cluster: &TestCluster,
    indexer_client: &HttpClient,
    address: IotaAddress,
    objects_count: u32,
) -> Vec<ObjectID> {
    let mut coins: Vec<ObjectID> = Vec::new();
    for _ in 0..objects_count {
        let coin = cluster
            .fund_address_and_return_gas(
                cluster.get_reference_gas_price().await,
                Some(FUNDED_BALANCE_PER_COIN),
                address,
            )
            .await;
        indexer_wait_for_object(indexer_client, coin.0, coin.1).await;
        coins.push(coin.0);
    }
    coins
}

/// Fetches a validator's IotaAddress from the latest IotaSystemStateSummary.
pub async fn get_validator(client: &HttpClient) -> IotaAddress {
    let iota_system_state = client.get_latest_iota_system_state_v2().await.unwrap();
    match iota_system_state {
        IotaSystemStateSummary::V1(v1) => v1.active_validators[0].iota_address,
        IotaSystemStateSummary::V2(v2) => v2.active_validators[0].iota_address,
        _ => panic!("unsupported IotaSystemStateSummary"),
    }
}

/// Fetches coin metadata from both fullnode and indexer for a given coin type.
pub async fn get_coin_metadata_fullnode_indexer(
    cluster: &TestCluster,
    client: &HttpClient,
    coin_type: String,
) -> (Option<IotaCoinMetadata>, Option<IotaCoinMetadata>) {
    let result_fullnode = cluster
        .rpc_client()
        .get_coin_metadata(coin_type.clone())
        .await
        .unwrap();
    let result_indexer = client.get_coin_metadata(coin_type).await.unwrap();
    (result_fullnode, result_indexer)
}
