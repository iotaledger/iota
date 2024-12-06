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
        let unswapped_addresses = self
            .addresses
            .values()
            .filter_map(|a| (!a.is_swapped()).then_some(a.address()))
            .collect::<Vec<_>>();
        if !unswapped_addresses.is_empty() {
            anyhow::bail!("unswapped addresses: {:?}", unswapped_addresses);
        }

        Ok(())
    }

    /// Converts a [`StardustAddress`] to an [`Owner`] by first
    /// converting it to an [`IotaAddress`] and then checking against the
    /// swap map for potential address substitutions.
    ///
    /// If the address exists in the swap map, it is swapped with the
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

    /// Converts a [`StardustAddress`] to an [`Owner`] by first
    /// converting it to an [`IotaAddress`] and then checking against the
    /// swap map for potential address substitutions.
    ///
    /// If the address exists in the swap map, it is swapped with the
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

    /// Converts a [`StardustAddress`] to an [`IotaAddress`] and
    /// checks against the swap map for potential address
    /// substitutions.
    ///
    /// If the address exists in the swap map, it is swapped with the
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
    /// # Example CSV File
    /// ```csv
    /// Origin,Destination
    /// iota1qp8h9augeh6tk3uvlxqfapuwv93atv63eqkpru029p6sgvr49eufyz7katr,0x7f1c8e2fdb9a5c348a4e793bc0a612b2879d4e5bc3a846b2f22c7a3f9b46d2ce
    /// iota1qp7h2lkjhs6tk3uvlxqfjhlfw34atv63eqkpru356p6sgvr76eufyz1opkh,0x42d8c182eb1f3b2366d353eed4eb02a31d1d7982c0fd44683811d7036be3a85e
    /// ```
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
    ///   parsed into an [`IotaAddress`].
    pub fn from_csv(file_path: &str) -> Result<AddressSwapMap, anyhow::Error> {
        let current_dir = std::env::current_dir()?;
        let file_path = current_dir.join(file_path);
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(true)
            .from_path(file_path)?;
        let mut addresses = HashMap::new();
        verify_headers(reader.headers()?)?;

        // Skip headers raw
        for result in reader.records().skip(1) {
            let record = result?;
            let origin: OriginAddress =
                stardust_to_iota_address(StardustAddress::try_from_bech32(&record[0])?)?;
            let destination: DestinationAddress = DestinationAddress::new(record[1].parse()?);
            addresses.insert(origin, destination);
        }

        Ok(AddressSwapMap { addresses })
    }
}

fn verify_headers(headers: &csv::StringRecord) -> Result<(), anyhow::Error> {
    const LEFT_HEADER: &str = "Origin";
    const RIGHT_HEADER: &str = "Destination";

    if &headers[0] != LEFT_HEADER && &headers[1] != RIGHT_HEADER {
        anyhow::bail!("Invalid CSV headers");
    }
    Ok(())
}
