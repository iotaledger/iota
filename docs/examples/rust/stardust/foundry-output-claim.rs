// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Example demonstrating the claim of a CoinManagerTreasuryCap related to a
//! foundry output. In order to work, it requires a network with test objects
//! generated from iota-genesis-builder/src/stardust/test_outputs.

use anyhow::anyhow;
use docs_examples::utils::{clean_keystore, fund_address, setup_keystore};
use iota_keys::keystore::AccountKeystore;
use iota_sdk::{
    rpc_types::{
        IotaObjectDataOptions, IotaObjectResponseQuery, IotaTransactionBlockResponseOptions,
    },
    types::{
        base_types::ObjectID,
        coin_manager::CoinManagerTreasuryCap,
        crypto::SignatureScheme::ED25519,
        dynamic_field::DynamicFieldName,
        gas_coin::GAS,
        programmable_transaction_builder::ProgrammableTransactionBuilder,
        quorum_driver_types::ExecuteTransactionRequestType,
        transaction::{Argument, ObjectArg, Transaction, TransactionData},
        TypeTag, IOTA_FRAMEWORK_ADDRESS, STARDUST_ADDRESS,
    },
    IotaClientBuilder,
};
use move_core_types::{ident_str, language_storage::StructTag};
use shared_crypto::intent::Intent;

/// Got from iota-genesis-builder/src/stardust/test_outputs/alias_ownership.rs
const MAIN_ADDRESS_MNEMONIC: &str = "few hood high omit camp keep burger give happy iron evolve draft few dawn pulp jazz box dash load snake gown bag draft car";

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // Build an IOTA client for a local network.
    let iota_client = IotaClientBuilder::default().build_localnet().await?;

    // Setup a temporary file based keystore.
    let mut keystore = setup_keystore()?;

    // Derive the address of the first account and set it as default.
    let sender = keystore.import_from_mnemonic(MAIN_ADDRESS_MNEMONIC, ED25519, None, None)?;

    println!("Sender address: {sender:?}");

    // Fund the sender address
    fund_address(&iota_client, &mut keystore, sender).await?;

    // Get a gas coin.
    let gas_coin = iota_client
        .coin_read_api()
        .get_coins(sender, None, None, None)
        .await?
        .data
        .into_iter()
        .next()
        .ok_or(anyhow!("No coins found for sponsor"))?;

    // This object id was fetched manually. It refers to an Alias Output object that
    // contains a CoinManagerTreasuryCap (i.e., a Foundry representation).
    let alias_output_object_id = ObjectID::from_hex_literal(
        "0xa58e9b6b85863e2fa50710c4594f701b2f5e2c6ff5e3c2b10cf09e6b18d740da",
    )?;
    let alias_output_object = iota_client
        .read_api()
        .get_object_with_options(
            alias_output_object_id,
            IotaObjectDataOptions::new().with_bcs(),
        )
        .await?
        .data
        .ok_or(anyhow!("alias output not found"))?;
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
    let alias_object_ref = alias_object.object_ref();

    // Get the objects owned by the alias object and filter in the ones with
    // CoinManagerTreasuryCap as type.
    let alias_owned_objects_page = iota_client
        .read_api()
        .get_owned_objects(
            alias_object_ref.0.into(),
            Some(IotaObjectResponseQuery::new_with_options(
                IotaObjectDataOptions::new().with_bcs().with_type(),
            )),
            None,
            None,
        )
        .await?;
    // Only one page should exist.
    assert!(!alias_owned_objects_page.has_next_page);
    // Get the CoinManagerTreasuryCaps from the query
    let owned_coin_manager_treasury_caps = alias_owned_objects_page
        .data
        .into_iter()
        .filter(|object| {
            CoinManagerTreasuryCap::is_coin_manager_treasury_cap(
                &object
                    .data
                    .as_ref()
                    .expect("the query should request the data")
                    .object_type()
                    .expect("should contain the type")
                    .try_into()
                    .expect("should convert into a struct tag"),
            )
        })
        .collect::<Vec<_>>();

    // Get only the first coin manager treasury cap
    let coin_manager_treasury_cap_object = owned_coin_manager_treasury_caps
        .into_iter()
        .next()
        .ok_or(anyhow!("no coin manager treasury caps found"))?
        .data
        .ok_or(anyhow!("coin manager treasury cap data not found"))?;
    let coin_manager_treasury_cap_object_ref = coin_manager_treasury_cap_object.object_ref();

    // Extract the foundry token type from the type parameters of the coin manager
    // treasury cap object
    let foundry_token_type_struct_tag: StructTag = coin_manager_treasury_cap_object
        .object_type()
        .expect("should contain the type")
        .try_into()?;
    let foundry_token_type = foundry_token_type_struct_tag
        .type_params
        .first()
        .expect("should contain the type param");

    // Create a PTB to claim the CoinManagerTreasuryCap related to the foundry
    // output from the alias output.
    let pt = {
        // Init a programmable transaction builder.
        let mut builder = ProgrammableTransactionBuilder::new();

        // Type argument for an AliasOutput coming from the IOTA network, i.e., the
        // IOTA token or the Gas type tag.
        let type_arguments = vec![GAS::type_tag()];
        // Then pass the AliasOutput object as an input.
        let arguments = vec![builder.obj(ObjectArg::ImmOrOwnedObject(alias_output_object_ref))?];
        // Finally call the alias_output::extract_assets function.
        if let Argument::Result(extracted_assets) = builder.programmable_move_call(
            STARDUST_ADDRESS.into(),
            ident_str!("alias_output").to_owned(),
            ident_str!("extract_assets").to_owned(),
            type_arguments,
            arguments,
        ) {
            // The alias output can always be unlocked by the governor address. So the
            // command will be successful and will return a `base_token` (i.e., IOTA)
            // balance, a `Bag` of the related native tokens and the related Alias object.
            let extracted_base_token = Argument::NestedResult(extracted_assets, 0);
            let extracted_native_tokens_bag = Argument::NestedResult(extracted_assets, 1);
            let extracted_alias = Argument::NestedResult(extracted_assets, 2);

            // Extract the IOTA balance.
            let type_arguments = vec![GAS::type_tag()];
            let arguments = vec![extracted_base_token];
            let iota_coin = builder.programmable_move_call(
                IOTA_FRAMEWORK_ADDRESS.into(),
                ident_str!("coin").to_owned(),
                ident_str!("from_balance").to_owned(),
                type_arguments,
                arguments,
            );

            // Transfer the IOTA balance to the sender.
            builder.transfer_arg(sender, iota_coin);

            // In this example the native tokens bag is empty, so it can be destroyed.
            let arguments = vec![extracted_native_tokens_bag];
            builder.programmable_move_call(
                IOTA_FRAMEWORK_ADDRESS.into(),
                ident_str!("bag").to_owned(),
                ident_str!("destroy_empty").to_owned(),
                vec![],
                arguments,
            );

            // Extract the CoinManagerTreasuryCap
            let type_arguments = vec![foundry_token_type.clone()];
            let arguments = vec![
                extracted_alias,
                builder.obj(ObjectArg::Receiving(coin_manager_treasury_cap_object_ref))?,
            ];
            let coin_manager_treasury_cap = builder.programmable_move_call(
                STARDUST_ADDRESS.into(),
                ident_str!("address_unlock_condition").to_owned(),
                ident_str!("unlock_alias_address_owned_coinmanager_treasury").to_owned(),
                type_arguments,
                arguments,
            );

            // Transfer the coin manager treasury cap.
            builder.transfer_arg(sender, coin_manager_treasury_cap);

            // Transfer the alias asset.
            builder.transfer_arg(sender, extracted_alias);
        }
        builder.finish()
    };

    // Setup a gas budget and a gas price.
    let gas_budget = 10_000_000;
    let gas_price = iota_client.read_api().get_reference_gas_price().await?;

    // Create a transaction data that will be sent to the network.
    let tx_data = TransactionData::new_programmable(
        sender,
        vec![gas_coin.object_ref()],
        pt,
        gas_budget,
        gas_price,
    );

    // Sign the transaction.
    let signature = keystore.sign_secure(&sender, &tx_data, Intent::iota_transaction())?;

    // Execute the transaction.
    let transaction_response = iota_client
        .quorum_driver_api()
        .execute_transaction_block(
            Transaction::from_data(tx_data, vec![signature]),
            IotaTransactionBlockResponseOptions::full_content(),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await?;

    println!("Transaction digest: {}", transaction_response.digest);

    // Finish and clean the temporary keystore file.
    clean_keystore()
}
