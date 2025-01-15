use std::collections::HashMap;

use iota_sdk::types::block::address::Address;
use iota_types::base_types::IotaAddress;

type OriginAddress = Address;
type Destination = (IotaAddress, u64, u64);

#[derive(Default)]
pub struct AddressSwapSplitMap {
    addresses: HashMap<OriginAddress, Destination>,
}

impl AddressSwapSplitMap {
    /// If the `address` passed as input is present in the map, then return
    /// a mutable reference to the destination, i.e., a tuple containing a
    /// destination address, a tokens target and a timelocked tokens target.
    pub fn get_destination_maybe_mut(
        &mut self,
        address: &OriginAddress,
    ) -> Option<&mut (IotaAddress, u64, u64)> {
        self.addresses.get_mut(address)
    }

    /// Check whether the map has all targets set to 0. Return the first
    /// occurrence of an entry where one or both the two targets are greater
    /// than zero. If none is found, then return None.
    pub fn validate_successfull_swap_split(
        &self,
    ) -> Option<(&OriginAddress, &IotaAddress, u64, u64)> {
        for (origin, (destination, tokens_target, tokens_timelocked_target)) in
            self.addresses.iter()
        {
            if *tokens_target > 0 || *tokens_timelocked_target > 0 {
                return Some((
                    origin,
                    destination,
                    *tokens_target,
                    *tokens_timelocked_target,
                ));
            }
        }
        None
    }

    /// Initializes an [`AddressSwapSplitMap`] by reading address pairs from a
    /// CSV file.
    ///
    /// The function expects the file to contain four columns: the origin
    /// address (first column), the destination address (second column), the
    /// tokens target (third column) and the timelocked tokens target
    /// (fourth column). These are parsed into a [`HashMap`] that maps
    /// origin addresses to tuples containing the destination address and
    /// the two targets.
    ///
    /// # Example CSV File
    /// ```csv
    /// Origin,Destination,Tokens,TokensTimelocked
    /// iota1qrukjnd6jhgwc0ls6dgt574sxuulcsmq5lnzhtv4jmlwkydhe2zvy69t7jj,0x1336d143de5eb55bcb069f55da5fc9f0c84e368022fd2bbe0125b1093b446313,107667149000,107667149000
    /// iota1qr4chj9jwhauvegqy40sdhj93mzmvc3mg9cmzlv2y6j8vpyxpvug2y6h5jd,0x83b5ed87bac715ecb09017a72d531ccc3c43bcb58edeb1ce383f1c46cfd79bec,388647312000,0

    /// ```
    /// 
    /// # Parameters
    /// - `file_path`: The relative path to the CSV file containing the address
    ///   mappings.
    ///
    /// # Returns
    /// - An [`AddressSwapSplitMap`] containing the parsed mappings.
    ///
    /// # Errors
    /// - Returns an error if the file cannot be found, read, or parsed
    ///   correctly.
    /// - Returns an error if the origin, destination addresses, or targets
    ///   cannot be parsed into.
    pub fn from_csv(file_path: &str) -> Result<AddressSwapSplitMap, anyhow::Error> {
        let current_dir = std::env::current_dir()?;
        let file_path = current_dir.join(file_path);
        let mut reader = csv::ReaderBuilder::new().from_path(file_path)?;
        let mut addresses = HashMap::new();

        verify_headers(reader.headers()?)?;

        for result in reader.records() {
            let record = result?;
            let origin = OriginAddress::try_from_bech32(&record[0])?;
            let destination_address = record[1].parse()?;
            let tokens_target = record[2].parse()?;
            let tokens_timelocked_target = record[3].parse()?;
            addresses.insert(
                origin,
                (destination_address, tokens_target, tokens_timelocked_target),
            );
        }

        Ok(AddressSwapSplitMap { addresses })
    }
}

fn verify_headers(headers: &csv::StringRecord) -> Result<(), anyhow::Error> {
    if headers.len() != 4
        || &headers[0] != "Origin"
        || &headers[1] != "Destination"
        || &headers[2] != "Tokens"
        || &headers[3] != "TokensTimelocked"
    {
        anyhow::bail!("Invalid CSV headers");
    }
    Ok(())
}
