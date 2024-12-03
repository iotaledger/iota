// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashMap, str::FromStr};

use serde::Deserialize;

use crate::base_types::IotaAddress;

type OriginAddress = IotaAddress;
type DestinationAddress = IotaAddress;

#[derive(Debug, Default, Deserialize)]
pub struct AddressSwapMap {
    addresses: HashMap<OriginAddress, DestinationAddress>,
    address_swapped_at_least_once: HashMap<OriginAddress, bool>,
}

impl AddressSwapMap {
    /// Retrieves the destination address for a given origin address.
    /// Marks the origin address as swapped if found.
    pub fn get_destination_address(
        &mut self,
        origin_address: OriginAddress,
    ) -> Option<&DestinationAddress> {
        self.addresses
            .get(&origin_address)
            .map(|destination_address| {
                // Mark the origin address as swapped
                if let Some(swapped_flag) =
                    self.address_swapped_at_least_once.get_mut(&origin_address)
                {
                    *swapped_flag = true;
                }
                destination_address
            })
    }

    /// Verifies that all addresses have been swapped at least once.
    /// Panics if any address is not swapped.
    pub fn verify_all_addresses_swapped(&self) {
        if !self
            .address_swapped_at_least_once
            .values()
            .all(|is_swapped| *is_swapped)
        {
            panic!("Not all addresses have been swapped");
        }
    }
}

pub fn init_address_swap_map(file_path: &str) -> Result<AddressSwapMap, anyhow::Error> {
    let current_dir = std::env::current_dir()?;
    let file_path = current_dir.join(file_path);
    let mut reader = csv::Reader::from_path(file_path)?;
    let mut addresses = HashMap::new();
    let mut address_swapped_at_least_once = HashMap::new();

    for result in reader.records() {
        let record = result?;
        let origin: OriginAddress = IotaAddress::from_str(&record[0])?;
        let destination: DestinationAddress = IotaAddress::from_str(&record[1])?;
        addresses.insert(origin, destination);
        address_swapped_at_least_once.insert(origin, false);
    }

    Ok(AddressSwapMap {
        addresses,
        address_swapped_at_least_once,
    })
}
