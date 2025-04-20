use std::str::FromStr;
use iota_sdk::types::base_types::IotaAddress;
use iota_sdk::IotaClientBuilder;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
   let iota = IotaClientBuilder::default().build_testnet().await?;
   let address = IotaAddress::from_str("0x849d63687330447431a2e76fecca4f3c10f6884ebaa9909674123c6c662612a3")?;
   let objects = iota.coin_read_api().get_balance(address, None).await?;
   println!("{objects:?}");
   Ok(())
}