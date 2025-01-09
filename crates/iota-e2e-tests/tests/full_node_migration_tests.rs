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
        types::address_swap_map::AddressSwapMap,
    },
};
use iota_json_rpc_types::{
    IotaObjectDataFilter, IotaObjectDataOptions, IotaObjectResponseQuery,
    IotaTransactionBlockResponse, IotaTransactionBlockResponseOptions,
};
use iota_keys::keystore::{AccountKeystore, FileBasedKeystore};
use iota_macros::sim_test;
use iota_sdk::IotaClient;
use iota_types::{
    IOTA_FRAMEWORK_ADDRESS, STARDUST_ADDRESS, TypeTag,
    base_types::{IotaAddress, ObjectID},
    crypto::SignatureScheme::ED25519,
    dynamic_field::DynamicFieldName,
    gas_coin::GAS,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    quorum_driver_types::ExecuteTransactionRequestType,
    stardust::{coin_type::CoinType, output::NftOutput},
    transaction::{Argument, ObjectArg, Transaction, TransactionData},
};
use move_core_types::ident_str;
use shared_crypto::intent::Intent;
use tempfile::tempdir;
use test_cluster::TestClusterBuilder;

const HORNET_SNAPSHOT_PATH: &str = "tests/migration/test_hornet_full_snapshot.bin";
const ADDRESS_SWAP_MAP_PATH: &str = "tests/migration/address_swap.csv";
const TEST_TARGET_NETWORK: &str = "alphanet-test";
const MIGRATION_DATA_FILE_NAME: &str = "stardust_object_snapshot.bin";
const DELEGATOR: &str = "0x4f72f788cdf4bb478cf9809e878e6163d5b351c82c11f1ea28750430752e7892";

/// Got from iota-genesis-builder/src/stardust/test_outputs/alias_ownership.rs
const MAIN_ADDRESS_MNEMONIC: &str = "few hood high omit camp keep burger give happy iron evolve draft few dawn pulp jazz box dash load snake gown bag draft car";
/// Got from iota-genesis-builder/src/stardust/test_outputs/stardust_mix.rs
const SPONSOR_ADDRESS_MNEMONIC: &str = "okay pottery arch air egg very cave cash poem gown sorry mind poem crack dawn wet car pink extra crane hen bar boring salt";

#[sim_test]
async fn test_full_node_load_migration_data() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();

    // Setup the temporary dir and create the writer for the stardust object
    // snapshot
    let dir = tempdir()?;
    let stardudst_object_snapshot_file_path = dir.path().join(MIGRATION_DATA_FILE_NAME);
    let object_snapshot_writer =
        BufWriter::new(File::create(&stardudst_object_snapshot_file_path)?);

    // Generate the stardust object snapshot
    genesis_builder_snapshot_generation(object_snapshot_writer)?;
    // Then load it
    let snapshot_source = SnapshotSource::Local(stardudst_object_snapshot_file_path);

    // A new test cluster can be spawn with the stardust object snapshot
    let test_cluster = TestClusterBuilder::new()
        .with_migration_data(vec![snapshot_source])
        .with_delegator(IotaAddress::from_str(DELEGATOR).unwrap())
        .build()
        .await;

    // Use a client to issue a test transaction
    let client = test_cluster.wallet.get_client().await.unwrap();
    let tx_response = address_unlock_condition(client).await?;
    let IotaTransactionBlockResponse {
        confirmed_local_execution,
        errors,
        ..
    } = tx_response;

    // The transaction must be successful
    assert!(confirmed_local_execution.unwrap());
    assert!(errors.is_empty());
    Ok(())
}

fn genesis_builder_snapshot_generation(
    object_snapshot_writer: impl Write,
) -> Result<(), anyhow::Error> {
    let mut snapshot_parser =
        HornetSnapshotParser::new::<false>(File::open(HORNET_SNAPSHOT_PATH)?)?;
    let total_supply = scale_amount_for_iota(snapshot_parser.total_supply()?)?;
    let target_network = MigrationTargetNetwork::from_str(TEST_TARGET_NETWORK)?;
    let coin_type = CoinType::Iota;
    let address_swap_map = AddressSwapMap::from_csv(ADDRESS_SWAP_MAP_PATH)?;

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
        snapshot_parser.outputs(),
        object_snapshot_writer,
    )?;

    Ok(())
}

async fn address_unlock_condition(
    iota_client: IotaClient,
) -> Result<IotaTransactionBlockResponse, anyhow::Error> {
    // Setup the temporary file based keystore
    let dir = tempdir()?;
    let keystore_path = dir.path().join(PathBuf::from("iotatempdb"));
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

    fund_address(&iota_client, &mut keystore, sender).await?;

    // Get a gas coin
    let gas_coin = iota_client
        .coin_read_api()
        .get_coins(sender, None, None, None)
        .await?
        .data
        .into_iter()
        .next()
        .ok_or(anyhow!("No coins found"))?;

    // This object id was fetched manually. It refers to an Alias Output object that
    // owns a NftOutput.
    let alias_output_object_id = ObjectID::from_hex_literal(
        "0xe6bf3ef78d57eb36d7959b64a272c3581cdaeb93a1f1bf1068651901e3b1e91a",
    )?;

    let alias_output_object = iota_client
        .read_api()
        .get_object_with_options(
            alias_output_object_id,
            IotaObjectDataOptions::new().with_bcs(),
        )
        .await?
        .data
        .into_iter()
        .next()
        .ok_or(anyhow!("Alias output not found"))?;

    let alias_output_object_ref = alias_output_object.object_ref();

    // Get the dynamic field owned by the Alias Output, i.e., only the Alias
    // object.
    // The dynamic field name for the Alias object is "alias", of type vector<u8>
    let df_name = DynamicFieldName {
        type_: TypeTag::Vector(Box::new(TypeTag::U8)),
        value: serde_json::Value::String("alias".to_string()),
    };
    let alias_object = iota_client
        .read_api()
        .get_dynamic_field_object(alias_output_object_id, df_name)
        .await?
        .data
        .ok_or(anyhow!("alias not found"))?;
    let alias_object_address = alias_object.object_ref().0;

    // Some objects are owned by the Alias object. In this case we filter them by
    // type using the NftOutput type.
    let owned_objects_query_filter =
        IotaObjectDataFilter::StructType(NftOutput::tag(GAS::type_tag()));
    let owned_objects_query = IotaObjectResponseQuery::new(Some(owned_objects_query_filter), None);

    // Get the first NftOutput found
    let nft_output_object_owned_by_alias = iota_client
        .read_api()
        .get_owned_objects(
            alias_object_address.into(),
            Some(owned_objects_query),
            None,
            None,
        )
        .await?
        .data
        .into_iter()
        .next()
        .ok_or(anyhow!("Owned nft outputs not found"))?
        .data
        .ok_or(anyhow!("Nft output data not found"))?;

    let nft_output_object_ref = nft_output_object_owned_by_alias.object_ref();

    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();

        // Extract alias output assets
        let type_arguments = vec![GAS::type_tag()];
        let arguments = vec![builder.obj(ObjectArg::ImmOrOwnedObject(alias_output_object_ref))?];
        if let Argument::Result(extracted_alias_output_assets) = builder.programmable_move_call(
            STARDUST_ADDRESS.into(),
            ident_str!("alias_output").to_owned(),
            ident_str!("extract_assets").to_owned(),
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
                IOTA_FRAMEWORK_ADDRESS.into(),
                ident_str!("coin").to_owned(),
                ident_str!("from_balance").to_owned(),
                type_arguments,
                arguments,
            );

            // Transfer the IOTA balance to the sender.
            builder.transfer_arg(sender, iota_coin);

            // Cleanup the bag.
            let arguments = vec![extracted_native_tokens_bag];
            builder.programmable_move_call(
                IOTA_FRAMEWORK_ADDRESS.into(),
                ident_str!("bag").to_owned(),
                ident_str!("destroy_empty").to_owned(),
                vec![],
                arguments,
            );

            // Unlock the nft output.
            let type_arguments = vec![GAS::type_tag()];
            let arguments = vec![
                alias,
                builder.obj(ObjectArg::Receiving(nft_output_object_ref))?,
            ];

            let nft_output = builder.programmable_move_call(
                STARDUST_ADDRESS.into(),
                ident_str!("address_unlock_condition").to_owned(),
                ident_str!("unlock_alias_address_owned_nft").to_owned(),
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
                STARDUST_ADDRESS.into(),
                ident_str!("nft_output").to_owned(),
                ident_str!("extract_assets").to_owned(),
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
                    IOTA_FRAMEWORK_ADDRESS.into(),
                    ident_str!("coin").to_owned(),
                    ident_str!("from_balance").to_owned(),
                    type_arguments,
                    arguments,
                );

                // Transfer the IOTA balance to the sender.
                builder.transfer_arg(sender, iota_coin);

                // Cleanup the bag because it is empty.
                let arguments = vec![extracted_native_tokens_bag];
                builder.programmable_move_call(
                    IOTA_FRAMEWORK_ADDRESS.into(),
                    ident_str!("bag").to_owned(),
                    ident_str!("destroy_empty").to_owned(),
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
    let gas_price = iota_client.read_api().get_reference_gas_price().await?;

    // Create the transaction data that will be sent to the network
    let tx_data = TransactionData::new_programmable(
        sender,
        vec![gas_coin.object_ref()],
        pt,
        gas_budget,
        gas_price,
    );

    // Sign the transaction
    let signature = keystore.sign_secure(&sender, &tx_data, Intent::iota_transaction())?;

    // Execute transaction
    let transaction_response = iota_client
        .quorum_driver_api()
        .execute_transaction_block(
            Transaction::from_data(tx_data, vec![signature]),
            IotaTransactionBlockResponseOptions::full_content(),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await?;

    Ok(transaction_response)
}

/// Utility function for funding an address using the transfer of a coin.
pub async fn fund_address(
    iota_client: &IotaClient,
    keystore: &mut FileBasedKeystore,
    recipient: IotaAddress,
) -> Result<(), anyhow::Error> {
    // Derive the address of the sponsor.
    let sponsor = keystore.import_from_mnemonic(SPONSOR_ADDRESS_MNEMONIC, ED25519, None, None)?;

    // Get a gas coin.
    let gas_coin = iota_client
        .coin_read_api()
        .get_coins(sponsor, None, None, None)
        .await?
        .data
        .into_iter()
        .next()
        .ok_or(anyhow!("No coins found for sponsor"))?;

    let pt = {
        // Init a programmable transaction builder.
        let mut builder = ProgrammableTransactionBuilder::new();
        // Pay all iotas from the gas object
        builder.pay_all_iota(recipient);
        builder.finish()
    };

    // Setup a gas budget and a gas price.
    let gas_budget = 10_000_000;
    let gas_price = iota_client.read_api().get_reference_gas_price().await?;

    // Create a transaction data that will be sent to the network.
    let tx_data = TransactionData::new_programmable(
        sponsor,
        vec![gas_coin.object_ref()],
        pt,
        gas_budget,
        gas_price,
    );

    // Sign the transaction.
    let signature = keystore.sign_secure(&sponsor, &tx_data, Intent::iota_transaction())?;

    // Execute the transaction.
    iota_client
        .quorum_driver_api()
        .execute_transaction_block(
            Transaction::from_data(tx_data, vec![signature]),
            IotaTransactionBlockResponseOptions::full_content(),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await?;

    Ok(())
}
