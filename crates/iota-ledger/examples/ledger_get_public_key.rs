// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::str::FromStr;

use anyhow::Result;
use clap::{Arg, Command};

pub fn main() -> Result<()> {
    let matches = Command::new("get_public_key")
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
            Arg::new("verify")
                .long("verify")
                .help("verify address (default false)")
                .action(clap::ArgAction::SetTrue)
                .required(false),
        )
        .arg(
            Arg::new("is-simulator")
                .short('s')
                .long("simulator")
                .help("select the simulator as transport")
                .action(clap::ArgAction::SetTrue)
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

    let verify = matches.get_flag("verify");

    let ledger = if is_simulator {
        iota_ledger::Ledger::new_with_simulator()?
    } else {
        iota_ledger::Ledger::new_with_native_hid()?
    };

    // generate address without prompt
    let pk_result = if verify {
        ledger.verify_address(&derivation_path)?
    } else {
        ledger.get_public_key(&derivation_path)?
    };

    println!("Public Key: 0x{}", hex::encode(&pk_result.public_key));
    println!("Address: 0x{}", hex::encode(pk_result.address));

    Ok(())
}
