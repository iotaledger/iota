// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;

pub fn main() -> Result<()> {
    let mut ledger = iota_ledger::Ledger::new_with_native_hid()?;
    ledger.ensure_app_is_open()?;
    let version = ledger.get_version()?;
    println!("Current IOTA app version: {version}");
    Ok(())
}
