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
        self.addresses.get(&origin_address).inspect(|_| {
            // Mark the origin address as swapped
            if let Some(swapped_flag) = self.address_swapped_at_least_once.get_mut(&origin_address)
            {
                *swapped_flag = true;
            }
        })
    }

    /// Verifies that all addresses have been swapped at least once.
    /// Returns an error if any address is not swapped.
    pub fn verify_all_addresses_swapped(&self) -> anyhow::Result<()> {
        if let Some((addr, _)) = self
            .address_swapped_at_least_once
            .iter()
            .find(|(_, &is_swapped)| !is_swapped)
        {
            return Err(anyhow::anyhow!("address to swap not found: {:?}", addr));
        }
        Ok(())
    }
}

/// Initializes an [`AddressSwapMap`] by reading address pairs from a CSV file.
///
/// The function expects the file to contain two columns: the origin address
/// (first column) and the destination address (second column). These are parsed
/// into a [`HashMap`] that maps origin addresses to destination addresses.
///
/// Additionally, a separate [`HashMap`] is created to track whether each origin
/// address has been swapped at least once, initialized to `false` for all
/// entries.
///
/// # Parameters
/// - `file_path`: The relative path to the CSV file containing the address
///   mappings.
///
/// # Returns
/// - An [`AddressSwapMap`] containing the parsed mappings and the tracking
///   state.
///
/// # Errors
/// - Returns an error if the file cannot be found, read, or parsed correctly.
/// - Returns an error if the origin or destination addresses cannot be parsed
///   into `IotaAddress`.
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
