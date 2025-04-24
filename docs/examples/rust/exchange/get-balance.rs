// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::str::FromStr;

use iota_sdk::{IotaClientBuilder, types::base_types::IotaAddress};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let iota = IotaClientBuilder::default()
        .build("https://api.devnet.iota.cafe:443")
        .await
        .unwrap();
    let address = IotaAddress::from_str(
        "0x849d63687330447431a2e76fecca4f3c10f6884ebaa9909674123c6c662612a3",
    )?;
    let objects = iota.coin_read_api().get_balance(address, None).await?;
    println!("{objects:?}");
    Ok(())
}
