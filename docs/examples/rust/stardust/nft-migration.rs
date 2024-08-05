// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Example demonstrating the transfer stardust::NFT to custom user's NFT.
//! In order to work, it requires a network with test objects
//! generated from iota-genesis-builder/src/stardust/test_outputs.

use std::{fs, path::PathBuf, str::FromStr};

use anyhow::anyhow;
use iota_keys::keystore::{AccountKeystore, FileBasedKeystore};
use iota_sdk::{
    rpc_types::{IotaData, IotaObjectDataOptions, IotaTransactionBlockResponseOptions},
    types::{
        base_types::ObjectID, crypto::SignatureScheme::ED25519, dynamic_field::DynamicFieldName, gas_coin::GAS, programmable_transaction_builder::ProgrammableTransactionBuilder, quorum_driver_types::ExecuteTransactionRequestType, stardust::output::{Nft, NftOutput}, transaction::{Argument, ObjectArg, Transaction, TransactionData}, TypeTag, IOTA_FRAMEWORK_ADDRESS, STARDUST_ADDRESS
    },
    IotaClientBuilder,
};
use move_core_types::{account_address::AccountAddress, ident_str};
use shared_crypto::intent::Intent;
/// Got from iota-genesis-builder/src/stardust/test_outputs/stardust_mix.rs
const MAIN_ADDRESS_MNEMONIC: &str = "okay pottery arch air egg very cave cash poem gown sorry mind poem crack dawn wet car pink extra crane hen bar boring salt";

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // Build an iota client for a local network
    let iota_client = IotaClientBuilder::default().build_localnet().await?;

    // Setup the temporary file based keystore
    let mut keystore = setup_keystore()?;

    // Derive the address of the first account and set it as default
    let sender = keystore.import_from_mnemonic(MAIN_ADDRESS_MNEMONIC, ED25519, None)?;

    println!("{sender:?}");

    // Get a gas coin
    let gas_coin = iota_client
        .coin_read_api()
        .get_coins(sender, None, None, None)
        .await?
        .data
        .into_iter()
        .next()
        .ok_or(anyhow!("No coins found"))?;

    // Get an NftOutput object
    let nft_output_object_id = ObjectID::from_hex_literal(
        "0x573f657e7021ce66e02c838bd14a05485aa0fe6e344228eb90b36a5bf3014d19",
    )?;

    let nft_output_object = iota_client
        .read_api()
        .get_object_with_options(
            nft_output_object_id,
            IotaObjectDataOptions::new().with_bcs(),
        )
        .await?
        .data
        .ok_or(anyhow!("Nft output not found"))?;

    let df_name = DynamicFieldName {
        type_: TypeTag::Vector(Box::new(TypeTag::U8)),
        value: serde_json::Value::String("nft".to_string()),
    };
    let nft_object_dyn_field = iota_client
        .read_api()
        .get_dynamic_field_object(nft_output_object_id, df_name)
        .await?
        .data
        .ok_or(anyhow!("nft not found"))?;
    
    let nft_object_id = nft_object_dyn_field.object_ref().0;


    let nft_object = iota_client
    .read_api()
    .get_object_with_options(
        nft_object_id,
        IotaObjectDataOptions::new().with_bcs(),
    )
    .await?
    .data
    .ok_or(anyhow!("Nft not found"))?;

    let nft = bcs::from_bytes::<Nft>(
        &nft_object
            .bcs
            .expect("should contain bcs")
            .try_as_move()
            .expect("should convert it to a move object")
            .bcs_bytes,
    )?;

    let nft_metadata = nft.immutable_metadata;

    // Create a PTB
    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();
        let name_arg = builder.pure(nft_metadata.name)?;
        let description_arg = builder.pure(nft_metadata.description.unwrap())?;
        let url_arg = builder.pure(nft_metadata.uri)?;
        builder.programmable_move_call(
            //TODO: package id should be taken from chain(not hardcoded)
            ObjectID::from_str("0xeb5a5b3761da04af060646f235d655f307eefe4f1499b3edbb726244e9f910da")?,
            ident_str!("random_nft").to_owned(),
            ident_str!("mint").to_owned(),
            vec![],
            vec![name_arg, description_arg, url_arg],
        );


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
