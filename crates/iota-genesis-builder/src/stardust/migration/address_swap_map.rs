use std::{collections::HashMap, error::Error, str::FromStr};

use fs_extra::dir::remove;
use iota_types::base_types::IotaAddress;
use serde::Deserialize;

type OriginAddress = IotaAddress;
type DestinationAddress = IotaAddress;

#[derive(Debug, Deserialize)]
pub struct AddressSwapMap {
    addresses: HashMap<OriginAddress, DestinationAddress>,
}

impl AddressSwapMap {
    pub fn get_destination_address(
        &self,
        origin_address: OriginAddress,
    ) -> Option<&DestinationAddress> {
        self.addresses.get(&origin_address)
    }
}

pub fn init_address_swap_map(file_path: &str) -> Result<AddressSwapMap, anyhow::Error> {
    let current_dir = std::env::current_dir()?;
    let file_path = current_dir.join(file_path);
    let mut reader = csv::Reader::from_path(file_path)?;
    let mut addresses = HashMap::new();

    for result in reader.records() {
        let record = result?;
        let origin: OriginAddress = IotaAddress::from_str(&record[0])?;
        let destination: DestinationAddress = IotaAddress::from_str(&record[1])?;
        addresses.insert(origin, destination);
    }

    Ok(AddressSwapMap { addresses })
}
