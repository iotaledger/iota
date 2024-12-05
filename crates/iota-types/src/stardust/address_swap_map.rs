// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{cell::RefCell, collections::HashMap, str::FromStr};

use iota_stardust_sdk::types::block::address::Address;
use serde::Deserialize;

use super::stardust_to_iota_address;
use crate::base_types::IotaAddress;
type OriginAddress = IotaAddress;
type DestinationAddress = IotaAddress;

#[derive(Debug, Default, Deserialize)]
pub struct AddressSwapMap {
    addresses: HashMap<OriginAddress, (DestinationAddress, RefCell<bool>)>,
}

impl AddressSwapMap {
    /// Retrieves the destination address for a given origin address.
    /// Marks the origin address as swapped if found.
    pub fn get_destination_address(
        &self,
        origin_address: OriginAddress,
    ) -> Option<&DestinationAddress> {
        self.addresses
            .get(&origin_address)
            .map(|(destination, swapped_flag)| {
                // Mark the origin address as swapped
                *swapped_flag.borrow_mut() = true;
                // Return a shared reference to the destination
                destination
            })
    }

    /// Verifies that all addresses have been swapped at least once.
    /// Returns an error if any address is not swapped.
    pub fn verify_all_addresses_swapped(&self) -> anyhow::Result<()> {
        let not_swapped_addresses: Vec<_> = self.addresses.iter().filter(|(_, (_, is_swapped))| !*is_swapped.borrow()).map(|(orig_addr, (_, _))| orig_addr).collect();
        if !not_swapped_addresses.is_empty()
        {
            return Err(anyhow::anyhow!("addresses to swap not found: {:?}", not_swapped_addresses));
        }
        Ok(())
    }
}

/// Initializes an [`AddressSwapMap`] by reading address pairs from a CSV file.
///
/// The function expects the file to contain two columns: the origin address
/// (first column) and the destination address (second column). These are parsed
/// into a [`HashMap`] that maps origin addresses to tuples containing the
/// destination address and a flag initialized to `false`.
///
/// # Parameters
/// - `file_path`: The relative path to the CSV file containing the address
///   mappings.
///
/// # Returns
/// - An [`AddressSwapMap`] containing the parsed mappings.
///
/// # Errors
/// - Returns an error if the file cannot be found, read, or parsed correctly.
/// - Returns an error if the origin or destination addresses cannot be parsed
///   into `IotaAddress`.
pub fn init_address_swap_map(file_path: &str) -> Result<AddressSwapMap, anyhow::Error> {
    let current_dir = std::env::current_dir()?;
    let file_path = current_dir.join(file_path);
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_path(file_path)?;

    let mut addresses = HashMap::new();
    verify_headers(reader.headers()?)?;

    //Skip headers raw
    for result in reader.records().skip(1) {
        let record = result?;
        let origin: OriginAddress =
            stardust_to_iota_address(Address::try_from_bech32(&record[0])?)?;
        let destination: DestinationAddress = IotaAddress::from_str(&record[1])?;
        addresses.insert(origin, (destination, RefCell::new(false)));
    }

    Ok(AddressSwapMap { addresses })
}

fn verify_headers(headers: &csv::StringRecord) -> Result<(), anyhow::Error> {
    const LEFT_HEADER: &str = "Origin";
    const RIGHT_HEADER: &str = "Destination";

    if &headers[0] != LEFT_HEADER && &headers[1] != RIGHT_HEADER {
        return Err(anyhow::anyhow!("Invalid CSV headers"));
    }
    Ok(())
}
