// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::str::FromStr;

use anyhow::Result;
use clap::{Arg, Command};
use iota_sdk_types::crypto::Intent;
use iota_types::{crypto::EncodeDecodeBase64, object::Object, transaction::TransactionData};

fn transaction_from_base64(b64: &str) -> TransactionData {
    let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64)
        .expect("Invalid base64 in transaction");
    bcs::from_bytes(&bytes).expect("Invalid bcs in transaction")
}

fn object_from_base64(b64: &str) -> Object {
    let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64)
        .expect("Invalid base64 in object");
    bcs::from_bytes(&bytes).expect("Invalid bcs in object")
}

pub fn main() -> Result<()> {
    let matches = Command::new("sign_tx")
        .version("1.0")
        .arg(
            Arg::new("bip32-path")
                .short('p')
                .long("path")
                .help("bip32 path to use (default \"m/44'/4218'/0'/0'/0'\")")
                .value_name("PATH")
                .required(false),
        )
        .arg(
            Arg::new("transaction")
                .long("tx")
                .help("transaction bytes in base64 format")
                .required(true),
        )
        .arg(
            Arg::new("is-simulator")
                .short('s')
                .long("simulator")
                .help("select the simulator as transport")
                .action(clap::ArgAction::SetTrue)
                .required(false),
        )
        .arg(
            Arg::new("objects")
                .long("objects")
                .help("A list of input objects in base64 format")
                .value_name("OBJECTS")
                .num_args(1..)
                .action(clap::ArgAction::Append)
                .required(false),
        )
        .get_matches();

    let is_simulator = matches.get_flag("is-simulator");

    let derivation_path = bip32::DerivationPath::from_str(
        matches
            .get_one::<String>("bip32-path")
            .map(|s| s.as_str())
            .unwrap_or("m/44'/4218'/0'/0'/0'"),
    )?;

    let objects: Vec<Object> = matches
        .get_many::<String>("objects")
        .map(|objs| objs.map(|o| object_from_base64(o)).collect())
        .unwrap_or_default();

    let ledger = if is_simulator {
        iota_ledger::Ledger::new_with_simulator()?
    } else {
        iota_ledger::Ledger::new_with_native_hid()?
    };

    let key_response = ledger.get_public_key(&derivation_path)?;
    println!("Address: {}", key_response.address);

    let transaction = transaction_from_base64(
        matches
            .get_one::<String>("transaction")
            .expect("Transaction bytes are required"),
    );

    let signature = ledger.sign_intent(
        &derivation_path,
        &key_response.address,
        Intent::iota_transaction(),
        &transaction,
        objects,
    )?;
    println!("Signature: {}", &signature.signature.encode_base64());

    Ok(())
}
