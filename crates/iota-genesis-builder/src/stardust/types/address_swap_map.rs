// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;

use iota_sdk::types::block::address::Address as StardustAddress;
use iota_types::{base_types::IotaAddress, object::Owner, stardust::stardust_to_iota_address};

type OriginAddress = IotaAddress;

#[derive(Debug)]
struct DestinationAddress {
    address: IotaAddress,
    swapped: bool,
}

impl DestinationAddress {
    fn new(address: IotaAddress) -> Self {
        Self {
            address,
            swapped: false,
        }
    }

    fn address(&self) -> IotaAddress {
        self.address
    }

    fn is_swapped(&self) -> bool {
        self.swapped
    }

    fn set_swapped(&mut self) {
        self.swapped = true;
    }
}

#[derive(Debug, Default)]
pub struct AddressSwapMap {
    addresses: HashMap<OriginAddress, DestinationAddress>,
}

impl AddressSwapMap {
    /// Retrieves the destination address for a given origin address.
    pub fn destination_address(&self, origin_address: &OriginAddress) -> Option<IotaAddress> {
        self.addresses
            .get(origin_address)
            .map(DestinationAddress::address)
    }

    /// Retrieves the destination address for a given origin address.
    /// Marks the origin address as swapped if found.
    fn swap_destination_address(&mut self, origin_address: &OriginAddress) -> Option<IotaAddress> {
        self.addresses.get_mut(origin_address).map(|destination| {
            // Mark the origin address as swapped
            destination.set_swapped();
            destination.address()
        })
    }

    /// Verifies that all addresses have been swapped at least once.
    /// Returns an error if any address is not swapped.
    pub fn verify_all_addresses_swapped(&self) -> anyhow::Result<()> {
        if let Some(addr) = self.addresses.values().find(|a| !a.is_swapped()) {
            return Err(anyhow::anyhow!(
                "address has not been swapped: {:?}",
                addr.address()
            ));
        }
        Ok(())
    }

    /// Converts a ["Stardust" `Address`](Address) to an [`Owner`] by first
    /// converting it to an [`IotaAddress`] and then checking against the
    /// provided `AddressSwapMap` for potential address substitutions.
    ///
    /// If the address exists in the `AddressSwapMap`, it is swapped with the
    /// mapped destination address before being wrapped into an [`Owner`].
    pub fn stardust_to_iota_address_owner(
        &self,
        stardust_address: impl Into<StardustAddress>,
    ) -> anyhow::Result<Owner> {
        let mut address = stardust_to_iota_address(stardust_address)?;
        if let Some(addr) = self.destination_address(&address) {
            address = addr;
        }
        Ok(Owner::AddressOwner(address))
    }

    /// Converts a ["Stardust" `Address`](Address) to an [`Owner`] by first
    /// converting it to an [`IotaAddress`] and then checking against the
    /// provided `AddressSwapMap` for potential address substitutions.
    ///
    /// If the address exists in the `AddressSwapMap`, it is swapped with the
    /// mapped destination address before being wrapped into an [`Owner`].
    pub fn swap_stardust_to_iota_address_owner(
        &mut self,
        stardust_address: impl Into<StardustAddress>,
    ) -> anyhow::Result<Owner> {
        let mut address = stardust_to_iota_address(stardust_address)?;
        if let Some(addr) = self.swap_destination_address(&address) {
            address = addr;
        }
        Ok(Owner::AddressOwner(address))
    }

    /// Converts a ["Stardust" `Address`](Address) to an [`IotaAddress`] and
    /// checks against the provided `AddressSwapMap` for potential address
    /// substitutions.
    ///
    /// If the address exists in the `AddressSwapMap`, it is swapped with the
    /// mapped destination address before being returned as an
    /// [`IotaAddress`].
    pub fn swap_stardust_to_iota_address(
        &mut self,
        stardust_address: impl Into<StardustAddress>,
    ) -> anyhow::Result<IotaAddress> {
        let mut address: IotaAddress = stardust_to_iota_address(stardust_address)?;
        if let Some(addr) = self.swap_destination_address(&address) {
            address = addr;
        }
        Ok(address)
    }

    /// Initializes an [`AddressSwapMap`] by reading address pairs from a CSV
    /// file.
    ///
    /// The function expects the file to contain two columns: the origin address
    /// (first column) and the destination address (second column). These are
    /// parsed into a [`HashMap`] that maps origin addresses to tuples
    /// containing the destination address and a flag initialized to
    /// `false`.
    ///
    /// # Parameters
    /// - `file_path`: The relative path to the CSV file containing the address
    ///   mappings.
    ///
    /// # Returns
    /// - An [`AddressSwapMap`] containing the parsed mappings.
    ///
    /// # Errors
    /// - Returns an error if the file cannot be found, read, or parsed
    ///   correctly.
    /// - Returns an error if the origin or destination addresses cannot be
    ///   parsed into `IotaAddress`.
    pub fn from_csv(file_path: &str) -> Result<AddressSwapMap, anyhow::Error> {
        let current_dir = std::env::current_dir()?;
        let file_path = current_dir.join(file_path);
        let mut reader = csv::Reader::from_path(file_path)?;
        let mut addresses = HashMap::new();

        for result in reader.records() {
            let record = result?;
            let origin: OriginAddress = record[0].parse()?;
            let destination: DestinationAddress = DestinationAddress::new(record[1].parse()?);
            addresses.insert(origin, destination);
        }

        Ok(AddressSwapMap { addresses })
    }
}
