use std::{fs, path::PathBuf, str::FromStr};

use anyhow::anyhow;
use bip32::DerivationPath;
use iota_keys::keystore::{AccountKeystore, FileBasedKeystore};
use iota_sdk::{
    rpc_types::{
        IotaObjectDataFilter, IotaObjectDataOptions, IotaObjectResponseQuery,
        IotaTransactionBlockResponseOptions,
    },
    types::{
        base_types::{ObjectID},
        crypto::SignatureScheme::ED25519,
        gas_coin::GAS,
        programmable_transaction_builder::ProgrammableTransactionBuilder,
        quorum_driver_types::ExecuteTransactionRequestType,
        stardust::output::{NftOutput},
        transaction::{Argument, ObjectArg, Transaction, TransactionData},
        IOTA_FRAMEWORK_ADDRESS, STARDUST_ADDRESS,
    },
    IotaClientBuilder,
};
use move_core_types::ident_str;
use shared_crypto::intent::Intent;

/// Got from iota-genesis-builder/src/stardust/test_outputs/alias_ownership.rs
const MAIN_ADDRESS_MNEMONIC: &str = "few hood high omit camp keep burger give happy iron evolve draft few dawn pulp jazz box dash load snake gown bag draft car";

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // Build an iota client for a local network
    let iota_client = IotaClientBuilder::default().build_localnet().await?;

    // Setup the temporary file based keystore
    let mut keystore = setup_keystore()?;

    // We're taking '/2' address of '0' account because it has AliasOutput asset
    let derivation_path = DerivationPath::from_str("m/44'/4218'/0'/0'/2'")?;
    println!("{derivation_path:?}");

    // Derive the address of the first account and set it as default
    let sender =
        keystore.import_from_mnemonic(MAIN_ADDRESS_MNEMONIC, ED25519, Some(derivation_path))?;

    // let addresses = keystore.addresses();
    println!("Sender address - {sender:?}");

    // Get a gas coin
    let gas_coin = iota_client
        .coin_read_api()
        .get_coins(sender, None, None, None)
        .await?
        .data
        .into_iter()
        .next()
        .ok_or(anyhow!("No coins found"))?;

    // Get an AliasOutput object
    let alias_output_object_id = ObjectID::from_hex_literal(
        "0x3b35e67750b8e4ccb45b2fc4a6a26a6d97e74c37a532f17177e6324ab93eaca6",
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

    let alias_dynamic_fields = iota_client
        .read_api()
        .get_dynamic_fields(alias_output_object_id, None, None)
        .await?
        .data
        .into_iter()
        .next()
        .ok_or(anyhow!("Alias output dynamic fields not found"))?;

    let alias_object_address = alias_dynamic_fields.object_id;

    let owned_objects_query_filter =
        IotaObjectDataFilter::StructType(NftOutput::tag(GAS::type_tag()));
    let owned_objects_query = IotaObjectResponseQuery::new(Some(owned_objects_query_filter), None);

    // Get NftOutput only
    if let Some(nft_output_object_owned_by_alias) = iota_client
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
    {
        let nft_output_object_ref = nft_output_object_owned_by_alias.object_ref();

        let pt = {
            let mut builder = ProgrammableTransactionBuilder::new();

            // Extract alias output assets
            let type_arguments = vec![GAS::type_tag()];
            let arguments =
                vec![builder.obj(ObjectArg::ImmOrOwnedObject(alias_output_object_ref))?];
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

                // Cleanup the alias.
                let arguments = vec![alias];
                builder.programmable_move_call(
                    STARDUST_ADDRESS.into(),
                    ident_str!("alias").to_owned(),
                    ident_str!("destroy").to_owned(),
                    vec![],
                    arguments,
                );

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
                    // If the nft output can be unlocked, the command will be succesful and will
                    // return a `base_token` (i.e., IOTA) balance and a `Bag` of native tokens and
                    // related nft object.
                    let extracted_base_token = Argument::NestedResult(extracted_assets, 0);
                    let extracted_native_tokens_bag = Argument::NestedResult(extracted_assets, 1);
                    let nft_asset = Argument::NestedResult(extracted_assets, 2);

                    let type_arguments = vec![GAS::type_tag()];
                    // Then pass the the bag and the receiver address as input
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

        println!("Transaction digest: {}", transaction_response.digest);
    }

    // Finish and clean the temporary keystore file
    clean_keystore()
}

fn setup_keystore() -> Result<FileBasedKeystore, anyhow::Error> {
    // Create a temporary keystore
    let keystore_path = PathBuf::from("iotatempdb");
    if !keystore_path.exists() {
        let keystore = FileBasedKeystore::new(&keystore_path)?;
        keystore.save()?;
    }
    // Read iota keystore
    FileBasedKeystore::new(&keystore_path)
}

fn clean_keystore() -> Result<(), anyhow::Error> {
    // Remove files
    fs::remove_file("iotatempdb")?;
    fs::remove_file("iotatempdb.aliases")?;
    Ok(())
}
