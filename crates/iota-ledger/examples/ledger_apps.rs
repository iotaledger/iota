// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{error::Error, thread, time};

use clap::{Arg, Command};

pub fn main() -> Result<(), Box<dyn Error>> {
    let matches = Command::new("apps")
        .version("1.0")
        .arg(
            Arg::new("is-simulator")
                .short('s')
                .long("simulator")
                .value_name("is_simulator")
                .help("select the simulator as transport")
                .action(clap::ArgAction::SetTrue)
                .required(false),
        )
        .get_matches();

    let is_simulator = matches.get_flag("is-simulator");

    let ledger = if is_simulator {
        iota_ledger::Ledger::new_with_simulator()?
    } else {
        iota_ledger::Ledger::new_with_native_hid()?
    };

    if ledger.is_app_open()? {
        println!("App is already open");
    } else {
        ledger.bolos_open_app()?;
        thread::sleep(time::Duration::from_secs(5));
    }
    let version = ledger.get_version()?;
    println!("current app version: {version}");
    Ok(())
}
