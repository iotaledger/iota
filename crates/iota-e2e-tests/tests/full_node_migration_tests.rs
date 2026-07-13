// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    fs::File,
    io::{BufWriter, Write},
    path::PathBuf,
    str::FromStr,
};

use anyhow::anyhow;
use bip32::DerivationPath;
use iota_genesis_builder::{
    SnapshotSource,
    stardust::{
        migration::{Migration, MigrationTargetNetwork},
        parse::HornetSnapshotParser,
        process_outputs::scale_amount_for_iota,
        types::{address_swap_map::AddressSwapMap, address_swap_split_map::AddressSwapSplitMap},
    },
};
use iota_grpc_client::{ReadMask, read_mask_fields::TransactionField};
use iota_json_rpc_types::IotaObjectDataFilter;
use iota_keys::keystore::{AccountKeystore, FileBasedKeystore};
use iota_macros::sim_test;
use iota_sdk_types::{
    Address, Argument, ExecutionStatus, Identifier, ObjectId, ObjectReference, StructTag, TypeTag,
    crypto::Intent,
};
use iota_types::{
    crypto::SignatureScheme::ED25519,
    effects::TransactionEffectsAPI,
    gas_coin::{GAS, GasCoin},
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    stardust::{coin_type::CoinType, output::NftOutput},
    transaction::{CallArg, Transaction, TransactionData, TransactionDataAPI},
};
use test_cluster::{TestCluster, TestClusterBuilder};

const HORNET_SNAPSHOT_PATH: &str = "tests/migration/test_hornet_full_snapshot.bin";
const ADDRESS_SWAP_MAP_PATH: &str = "tests/migration/address_swap.csv";
const ADDRESS_SWAP_SPLIT_MAP_PATH: &str = "tests/migration/swap_split.csv";
const TEST_TARGET_NETWORK: &str = "alphanet-test";
const MIGRATION_DATA_FILE_NAME: &str = "stardust_object_snapshot.bin";
const DELEGATOR: &str = "0x4f72f788cdf4bb478cf9809e878e6163d5b351c82c11f1ea28750430752e7892";

/// Got from iota-genesis-builder/src/stardust/test_outputs/alias_ownership.rs
const MAIN_ADDRESS_MNEMONIC: &str = "few hood high omit camp keep burger give happy iron evolve draft few dawn pulp jazz box dash load snake gown bag draft car";
/// Got from iota-genesis-builder/src/stardust/test_outputs/stardust_mix.rs
const SPONSOR_ADDRESS_MNEMONIC: &str = "okay pottery arch air egg very cave cash poem gown sorry mind poem crack dawn wet car pink extra crane hen bar boring salt";

/// Read objects owned by `owner` (optionally filtered by type) from node
/// state.
async fn owned_objects(
    test_cluster: &TestCluster,
    owner: Address,
    type_filter: Option<StructTag>,
) -> Vec<iota_types::object::Object> {
    // Read from node state rather than the gRPC StateService: the latter's
    // owned-object index is built from processed checkpoints and may not (yet)
    // contain the genesis/migration-loaded objects these tests rely on.
    test_cluster
        .fullnode_handle
        .iota_node
        .with_async(|node| async move {
            let filter = type_filter.map(IotaObjectDataFilter::StructType);
            let limit = 1000;
            let infos = node
                .state()
                .get_owner_objects(owner, None, limit, filter)
                .expect("owned-object lookup should succeed");
            assert!(
                infos.len() < limit,
                "owned-object lookup hit the page limit; results would be truncated"
            );
            let mut objects = Vec::new();
            for info in infos {
                if let Some(object) = node.state().get_object(&info.object_id) {
                    objects.push(object);
                }
            }
            objects
        })
        .await
}

/// The first IOTA gas coin owned by `owner`. Polls the node index, since gRPC
/// execution can return before the fullnode has indexed a just-funded coin.
async fn first_gas_coin_ref(
    test_cluster: &TestCluster,
    owner: Address,
) -> Result<ObjectReference, anyhow::Error> {
    for _ in 0..50 {
        if let Some(gas_coin_ref) = owned_objects(test_cluster, owner, None)
            .await
            .into_iter()
            .find_map(|object| GasCoin::try_from(&object).ok().map(|_| object.object_ref()))
        {
            return Ok(gas_coin_ref);
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
    Err(anyhow!("No coins found for {owner}"))
}

/// Execute a signed transaction over the node gRPC API and assert it succeeded.
async fn execute_and_assert_success(
    test_cluster: &TestCluster,
    transaction: Transaction,
) -> Result<(), anyhow::Error> {
    let signed: iota_sdk_types::SignedTransaction = transaction.try_into()?;
    let response = test_cluster
        .grpc_client()
        .execute_transaction(
            signed,
            Some(ReadMask::from(TransactionField::EFFECTS_BCS)),
            None,
        )
        .await?;
    let effects: iota_types::effects::TransactionEffects = response
        .body()
        .effects
        .as_ref()
        .expect("effects should be present")
        .bcs
        .as_ref()
        .expect("effects bcs should be present")
        .deserialize()?;
    anyhow::ensure!(
        matches!(effects.status(), ExecutionStatus::Success),
        "transaction failed: {:?}",
        effects.status()
    );
    Ok(())
}

#[sim_test]
async fn test_full_node_load_migration_data_with_address_swap() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();

    // Setup the temporary dir and create the writer for the stardust object
    // snapshot
    let tmp_dir = iota_common::tempdir();
    let stardudst_object_snapshot_file_path = tmp_dir.path().join(MIGRATION_DATA_FILE_NAME);
    let object_snapshot_writer =
        BufWriter::new(File::create(&stardudst_object_snapshot_file_path)?);

    // Get the address swap map
    let address_swap_map = AddressSwapMap::from_csv(ADDRESS_SWAP_MAP_PATH)?;

    // Generate the stardust object snapshot
    genesis_builder_snapshot_generation(
        object_snapshot_writer,
        address_swap_map,
        AddressSwapSplitMap::default(),
    )?;
    // Then load it
    let snapshot_source = SnapshotSource::Local(stardudst_object_snapshot_file_path);

    // A new test cluster can be spawn with the stardust object snapshot
    let test_cluster = TestClusterBuilder::new()
        // The tx response requests full content (balance/object changes), which
        // reads input objects at their pre-transaction versions; disable pruning
        // so those versions are not pruned before the response is built.
        .disable_fullnode_pruning()
        .with_migration_data(vec![snapshot_source])
        .with_delegator(Address::from_str(DELEGATOR).unwrap())
        .with_fullnode_enable_grpc_api(true)
        .build()
        .await;

    // Issue a test transaction over the node gRPC API; it must succeed.
    address_unlock_condition(&test_cluster).await?;
    Ok(())
}

#[sim_test]
async fn test_full_node_load_migration_data_with_address_swap_split() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();

    // Setup the temporary dir and create the writer for the stardust object
    // snapshot
    let tmp_dir = iota_common::tempdir();
    let stardudst_object_snapshot_file_path = tmp_dir.path().join(MIGRATION_DATA_FILE_NAME);
    let object_snapshot_writer =
        BufWriter::new(File::create(&stardudst_object_snapshot_file_path)?);

    // Get the address swap split map
    let address_swap_split_map = AddressSwapSplitMap::from_csv(ADDRESS_SWAP_SPLIT_MAP_PATH)?;

    // Generate the stardust object snapshot
    genesis_builder_snapshot_generation(
        object_snapshot_writer,
        AddressSwapMap::default(),
        address_swap_split_map.clone(),
    )?;
    // Then load it
    let snapshot_source = SnapshotSource::Local(stardudst_object_snapshot_file_path);

    // A new test cluster can be spawn with the stardust object snapshot
    let test_cluster = TestClusterBuilder::new()
        // The tx response requests full content (balance/object changes), which
        // reads input objects at their pre-transaction versions; disable pruning
        // so those versions are not pruned before the response is built.
        .disable_fullnode_pruning()
        .with_migration_data(vec![snapshot_source])
        .with_delegator(Address::from_str(DELEGATOR).unwrap())
        .with_fullnode_enable_grpc_api(true)
        .build()
        .await;

    check_address_swap_split_map_after_migration(&test_cluster, address_swap_split_map).await?;

    Ok(())
}

fn genesis_builder_snapshot_generation(
    object_snapshot_writer: impl Write,
    address_swap_map: AddressSwapMap,
    address_swap_split_map: AddressSwapSplitMap,
) -> Result<(), anyhow::Error> {
    let mut snapshot_parser =
        HornetSnapshotParser::new::<false>(File::open(HORNET_SNAPSHOT_PATH)?)?;
    let total_supply = scale_amount_for_iota(snapshot_parser.total_supply()?)?;
    let target_network = MigrationTargetNetwork::from_str(TEST_TARGET_NETWORK)?;
    let coin_type = CoinType::Iota;

    // Migrate using the parser output stream
    Migration::new(
        snapshot_parser.target_milestone_timestamp(),
        total_supply,
        target_network,
        coin_type,
        address_swap_map,
    )?
    .run_for_iota(
        snapshot_parser.target_milestone_timestamp(),
        address_swap_split_map,
        snapshot_parser.outputs(),
        object_snapshot_writer,
    )?;

    Ok(())
}

async fn address_unlock_condition(test_cluster: &TestCluster) -> Result<(), anyhow::Error> {
    // Setup the temporary file based keystore
    let tmp_dir = iota_common::tempdir();
    let keystore_path = tmp_dir.path().join(PathBuf::from("iotatempdb"));
    let mut keystore = FileBasedKeystore::new(&keystore_path)?;

    // For this example we need to derive an address that is not at index 0. This
    // because we need an alias output that owns an Nft Output. In this case, we can
    // derive the address index "/2'" of the "/0'" account.
    let derivation_path = DerivationPath::from_str("m/44'/4218'/0'/0'/2'")?;

    // Derive the address of the first account and set it as default
    let sender = keystore.import_from_mnemonic(
        MAIN_ADDRESS_MNEMONIC,
        ED25519,
        Some(derivation_path),
        None,
    )?;

    fund_address(test_cluster, &mut keystore, sender).await?;

    // Get a gas coin
    let gas_coin_ref = first_gas_coin_ref(test_cluster, sender).await?;

    // This object id was fetched manually. It refers to an Alias Output object that
    // owns a NftOutput.
    let alias_output_object_id =
        ObjectId::from_hex("0xe6bf3ef78d57eb36d7959b64a272c3581cdaeb93a1f1bf1068651901e3b1e91a")?;

    let alias_output_object_ref = test_cluster
        .fullnode_handle
        .iota_node
        .with_async(|node| async move {
            node.state()
                .get_object(&alias_output_object_id)
                .map(|object| object.object_ref())
        })
        .await
        .ok_or(anyhow!("Alias output not found"))?;

    // Get the dynamic field owned by the Alias Output, i.e., only the Alias
    // object. The dynamic field name for the Alias object is "alias", of type
    // vector<u8>.
    let alias_name_bcs_bytes = bcs::to_bytes(&b"alias".to_vec())?;
    let alias_object_id = test_cluster
        .fullnode_handle
        .iota_node
        .with_async(|node| async move {
            node.state()
                .get_dynamic_field_object_id(
                    alias_output_object_id,
                    TypeTag::Vector(Box::new(TypeTag::U8)),
                    &alias_name_bcs_bytes,
                )
                .expect("dynamic field lookup should succeed")
        })
        .await
        .ok_or(anyhow!("alias not found"))?;

    // Some objects are owned by the Alias object. In this case we filter them by
    // type using the NftOutput type.
    let nft_output_object_ref = owned_objects(
        test_cluster,
        alias_object_id.into(),
        Some(NftOutput::tag(GAS::type_tag())),
    )
    .await
    .into_iter()
    .next()
    .ok_or(anyhow!("Owned nft outputs not found"))?
    .object_ref();

    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();

        // Extract alias output assets
        let type_arguments = vec![GAS::type_tag()];
        let arguments = vec![builder.obj(CallArg::ImmutableOrOwned(alias_output_object_ref))?];
        if let Argument::Result(extracted_alias_output_assets) = builder.programmable_move_call(
            ObjectId::STARDUST,
            Identifier::from_static("alias_output"),
            Identifier::from_static("extract_assets"),
            type_arguments,
            arguments,
        ) {
            let extracted_base_token = Argument::NestedResult(extracted_alias_output_assets, 0);
            let extracted_native_tokens_bag =
                Argument::NestedResult(extracted_alias_output_assets, 1);
            let alias = Argument::NestedResult(extracted_alias_output_assets, 2);

            let type_arguments = vec![GAS::type_tag()];
            let arguments = vec![extracted_base_token];

            // Extract the IOTA balance.
            let iota_coin = builder.programmable_move_call(
                ObjectId::FRAMEWORK,
                Identifier::COIN_MODULE,
                Identifier::from_static("from_balance"),
                type_arguments,
                arguments,
            );

            // Transfer the IOTA balance to the sender.
            builder.transfer_arg(sender, iota_coin);

            // Cleanup the bag.
            let arguments = vec![extracted_native_tokens_bag];
            builder.programmable_move_call(
                ObjectId::FRAMEWORK,
                Identifier::BAG_MODULE,
                Identifier::from_static("destroy_empty"),
                vec![],
                arguments,
            );

            // Unlock the nft output.
            let type_arguments = vec![GAS::type_tag()];
            let arguments = vec![
                alias,
                builder.obj(CallArg::Receiving(nft_output_object_ref))?,
            ];

            let nft_output = builder.programmable_move_call(
                ObjectId::STARDUST,
                Identifier::from_static("address_unlock_condition"),
                Identifier::from_static("unlock_alias_address_owned_nft"),
                type_arguments,
                arguments,
            );

            // Transferring alias asset
            builder.transfer_arg(sender, alias);

            // Extract nft assets(base token, native tokens bag, nft asset itself).
            let type_arguments = vec![GAS::type_tag()];
            let arguments = vec![nft_output];
            // Finally call the nft_output::extract_assets function
            if let Argument::Result(extracted_assets) = builder.programmable_move_call(
                ObjectId::STARDUST,
                Identifier::from_static("nft_output"),
                Identifier::from_static("extract_assets"),
                type_arguments,
                arguments,
            ) {
                // If the nft output can be unlocked, the command will be successful and will
                // return a `base_token` (i.e., IOTA) balance and a `Bag` of native tokens and
                // related nft object.
                let extracted_base_token = Argument::NestedResult(extracted_assets, 0);
                let extracted_native_tokens_bag = Argument::NestedResult(extracted_assets, 1);
                let nft_asset = Argument::NestedResult(extracted_assets, 2);

                let type_arguments = vec![GAS::type_tag()];
                let arguments = vec![extracted_base_token];

                // Extract the IOTA balance.
                let iota_coin = builder.programmable_move_call(
                    ObjectId::FRAMEWORK,
                    Identifier::COIN_MODULE,
                    Identifier::from_static("from_balance"),
                    type_arguments,
                    arguments,
                );

                // Transfer the IOTA balance to the sender.
                builder.transfer_arg(sender, iota_coin);

                // Cleanup the bag because it is empty.
                let arguments = vec![extracted_native_tokens_bag];
                builder.programmable_move_call(
                    ObjectId::FRAMEWORK,
                    Identifier::BAG_MODULE,
                    Identifier::from_static("destroy_empty"),
                    vec![],
                    arguments,
                );

                // Transferring nft asset
                builder.transfer_arg(sender, nft_asset);
            }
        }
        builder.finish()
    };

    // Setup gas budget and gas price
    let gas_budget = 10_000_000;
    let gas_price = test_cluster.get_reference_gas_price().await;

    // Create the transaction data that will be sent to the network
    let tx_data =
        TransactionData::new_programmable(sender, vec![gas_coin_ref], pt, gas_budget, gas_price);

    // Sign the transaction
    let signature = keystore.sign_secure(&sender, &tx_data, Intent::iota_transaction())?;

    // Execute the transaction over the node gRPC API.
    execute_and_assert_success(
        test_cluster,
        Transaction::from_data(tx_data, vec![signature]),
    )
    .await
}

/// Utility function for funding an address using the transfer of a coin.
pub async fn fund_address(
    test_cluster: &TestCluster,
    keystore: &mut FileBasedKeystore,
    recipient: Address,
) -> Result<(), anyhow::Error> {
    // Derive the address of the sponsor.
    let sponsor = keystore.import_from_mnemonic(SPONSOR_ADDRESS_MNEMONIC, ED25519, None, None)?;

    // Get a gas coin.
    let gas_coin_ref = first_gas_coin_ref(test_cluster, sponsor).await?;

    let pt = {
        // Init a programmable transaction builder.
        let mut builder = ProgrammableTransactionBuilder::new();
        // Pay all iotas from the gas object
        builder.pay_all_iota(recipient);
        builder.finish()
    };

    // Setup a gas budget and a gas price.
    let gas_budget = 10_000_000;
    let gas_price = test_cluster.get_reference_gas_price().await;

    // Create a transaction data that will be sent to the network.
    let tx_data =
        TransactionData::new_programmable(sponsor, vec![gas_coin_ref], pt, gas_budget, gas_price);

    // Sign the transaction.
    let signature = keystore.sign_secure(&sponsor, &tx_data, Intent::iota_transaction())?;

    // Execute the transaction over the node gRPC API.
    execute_and_assert_success(
        test_cluster,
        Transaction::from_data(tx_data, vec![signature]),
    )
    .await
}

async fn check_address_swap_split_map_after_migration(
    test_cluster: &TestCluster,
    address_swap_split_map: AddressSwapSplitMap,
) -> Result<(), anyhow::Error> {
    for destinations in address_swap_split_map.map().values() {
        for (destination, tokens, tokens_timelocked) in destinations {
            if *tokens > 0 {
                let balance: u128 = owned_objects(test_cluster, *destination, None)
                    .await
                    .iter()
                    .filter_map(|object| {
                        GasCoin::try_from(object)
                            .ok()
                            .map(|coin| coin.value() as u128)
                    })
                    .sum();
                assert_eq!(balance, (*tokens as u128));
            }
            if *tokens_timelocked > 0 {
                let total: u64 = owned_objects(
                    test_cluster,
                    *destination,
                    Some(StructTag::new_timelocked_gas_balance()),
                )
                .await
                .iter()
                .filter_map(|object| object.as_timelock_balance_maybe())
                .map(|timelock| timelock.locked().value())
                .sum();
                assert_eq!(total, *tokens_timelocked);
            }
        }
    }
    Ok(())
}
