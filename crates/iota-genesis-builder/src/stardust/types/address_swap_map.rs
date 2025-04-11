// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashMap, fs::File};

use iota_config::genesis::csv_reader_with_comments;
use iota_sdk::types::block::address::Address as StardustAddress;
use iota_types::{base_types::IotaAddress, object::Owner, stardust::stardust_to_iota_address};

type OriginAddress = IotaAddress;

#[derive(Debug)]
pub struct DestinationAddress {
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
    /// iota1qp8h9augeh6tk3uvlxqfapuwv93atv63eqkpru029p6sgvr49eufyz7katr,0xa12b4d6ec3f9a28437d5c8f3e96ba72d3c4e8f5ac98d17b1a3b8e9f2c71d4a3c
    /// iota1qp7h2lkjhs6tk3uvlxqfjhlfw34atv63eqkpru356p6sgvr76eufyz1opkh,0x42d8c182eb1f3b2366d353eed4eb02a31d1d7982c0fd44683811d7036be3a85e
    /// ```
    ///
    /// Comments are optional, and start with a `#` character.
    /// Only entries that start with this character are treated as comments.
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
        let mut reader = csv_reader_with_comments(File::open(file_path)?);
        let mut addresses = HashMap::new();

        verify_headers(reader.headers()?)?;

        for result in reader.records() {
            let record = result?;
            let origin: OriginAddress =
                stardust_to_iota_address(StardustAddress::try_from_bech32(&record[0])?)?;
            let destination: DestinationAddress = DestinationAddress::new(record[1].parse()?);
            addresses.insert(origin, destination);
        }

        Ok(AddressSwapMap { addresses })
    }

    #[cfg(test)]
    pub fn addresses(&self) -> &HashMap<OriginAddress, DestinationAddress> {
        &self.addresses
    }
}

fn verify_headers(headers: &csv::StringRecord) -> Result<(), anyhow::Error> {
    anyhow::ensure!(
        headers.len() == 2 && &headers[0] == "Origin" && &headers[1] == "Destination",
        "Invalid CSV headers"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use tempfile::NamedTempFile;

    use crate::stardust::types::address_swap_map::AddressSwapMap;

    fn write_temp_file(content: &str) -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "{}", content).unwrap();
        file
    }

    #[test]
    fn test_from_csv_valid_file() {
        let content = "Origin,Destination\n\
                       iota1qp8h9augeh6tk3uvlxqfapuwv93atv63eqkpru029p6sgvr49eufyz7katr,0xa12b4d6ec3f9a28437d5c8f3e96ba72d3c4e8f5ac98d17b1a3b8e9f2c71d4a3c";
        let file = write_temp_file(content);
        let file_path = file.path().to_str().unwrap();
        let result = AddressSwapMap::from_csv(file_path);
        assert!(result.is_ok());
        let map = result.unwrap();
        assert_eq!(map.addresses().len(), 1);
    }

    #[test]
    fn test_from_csv_missing_file() {
        let result = AddressSwapMap::from_csv("nonexistent_file.csv");
        assert!(result.is_err());
    }

    #[test]
    fn test_from_csv_invalid_headers() {
        let content = "wrong_header1,wrong_header2\n\
                       iota1qp8h9augeh6tk3uvlxqfapuwv93atv63eqkpru029p6sgvr49eufyz7katr,0xa12b4d6ec3f9a28437d5c8f3e96ba72d3c4e8f5ac98d17b1a3b8e9f2c71d4a3c";
        let file = write_temp_file(content);
        let file_path = file.path().to_str().unwrap();
        let result = AddressSwapMap::from_csv(file_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_from_csv_invalid_record() {
        let content = "Origin,Destination\n\
                       iota1qp8h9augeh6tk3uvlxqfapuwv93atv63eqkpru029p6sgvr49eufyz7katr,0xa12b4d6ec3f9a28437d5c8f3e96ba72d3c4e8f5ac98d17b1a3b8e9f2c71d4a3c,invalid_number";
        let file = write_temp_file(content);
        let file_path = file.path().to_str().unwrap();
        let result = AddressSwapMap::from_csv(file_path);

        assert!(result.is_err());
    }

    #[test]
    fn test_from_csv_empty_file() {
        let content = "Origin,Destination";
        let file = write_temp_file(content);
        let file_path = file.path().to_str().unwrap();
        let result = AddressSwapMap::from_csv(file_path);
        assert!(result.is_ok());
        let map = result.unwrap();

        assert_eq!(map.addresses().len(), 0);
    }
}
