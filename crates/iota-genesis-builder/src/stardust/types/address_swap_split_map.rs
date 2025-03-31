// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashMap, fs::File};

use iota_config::genesis::csv_reader_with_comments;
use iota_sdk::types::block::address::Address;
use iota_types::base_types::IotaAddress;

type OriginAddress = Address;
type Destination = (IotaAddress, u64, u64);

#[derive(Clone, Debug, Default)]
pub struct AddressSwapSplitDestinations {
    destinations: Vec<Destination>,
}

impl AddressSwapSplitDestinations {
    /// Iterate over mutable destinations filtered by `tokens_target > 0`.
    pub fn iter_by_tokens_target_mut_filtered(
        &mut self,
    ) -> impl Iterator<Item = (&mut IotaAddress, &mut u64)> {
        self.destinations
            .iter_mut()
            .filter_map(|(destination, tokens_target, _)| {
                if *tokens_target > 0 {
                    Some((destination, tokens_target))
                } else {
                    None
                }
            })
    }

    /// Iterate over mutable destinations filtered by `tokens_timelocked_target
    /// > 0`.
    pub fn iter_by_tokens_timelocked_target_mut_filtered(
        &mut self,
    ) -> impl Iterator<Item = (&mut IotaAddress, &mut u64)> {
        self.destinations
            .iter_mut()
            .filter_map(|(destination, _, tokens_timelocked_target)| {
                if *tokens_timelocked_target > 0 {
                    Some((destination, tokens_timelocked_target))
                } else {
                    None
                }
            })
    }

    /// Returns true only if the destinations contains at least one
    /// tokens_timelocked_target that is greater than 0.
    pub fn contains_tokens_timelocked_target(&self) -> bool {
        self.destinations
            .iter()
            .any(|&(_, _, tokens_timelocked_target)| tokens_timelocked_target > 0)
    }
}

impl<'a> IntoIterator for &'a AddressSwapSplitDestinations {
    type Item = &'a Destination;
    type IntoIter = std::slice::Iter<'a, Destination>;

    fn into_iter(self) -> Self::IntoIter {
        self.destinations.iter()
    }
}

#[derive(Clone, Debug, Default)]
pub struct AddressSwapSplitMap {
    map: HashMap<OriginAddress, AddressSwapSplitDestinations>,
}

impl AddressSwapSplitMap {
    /// If the `address` passed as input is present in the map, then return
    /// a mutable reference to the destination, i.e., a tuple containing a
    /// destination address, a tokens target and a timelocked tokens target.
    pub fn get_destination_maybe_mut(
        &mut self,
        address: &OriginAddress,
    ) -> Option<&mut AddressSwapSplitDestinations> {
        self.map.get_mut(address)
    }

    /// Check whether the map has all targets set to 0. Return the first
    /// occurrence of an entry where one or both the two targets are greater
    /// than zero. If none is found, then return None.
    pub fn validate_successful_swap_split(
        &self,
    ) -> Option<(&OriginAddress, &IotaAddress, u64, u64)> {
        for (origin, destinations) in self.map.iter() {
            for (destination, tokens_target, tokens_timelocked_target) in destinations {
                if *tokens_target > 0 || *tokens_timelocked_target > 0 {
                    return Some((
                        origin,
                        destination,
                        *tokens_target,
                        *tokens_timelocked_target,
                    ));
                }
            }
        }
        None
    }

    /// Get the map.
    pub fn map(&self) -> &HashMap<OriginAddress, AddressSwapSplitDestinations> {
        &self.map
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
    /// Comments are optional, and start with a `#` character.
    /// Only entries that start with this character are treated as comments.
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
        let mut reader = csv_reader_with_comments(File::open(file_path)?);
        let mut address_swap_split_map: AddressSwapSplitMap = Default::default();

        let headers = reader.headers()?;
        anyhow::ensure!(
            headers.len() == 4
                && &headers[0] == "Origin"
                && &headers[1] == "Destination"
                && &headers[2] == "Tokens"
                && &headers[3] == "TokensTimelocked",
            "Invalid CSV headers"
        );

        for result in reader.records() {
            let record = result?;
            let origin = OriginAddress::try_from_bech32(&record[0])?;
            let destination_address = record[1].parse()?;
            let tokens_target = record[2].parse()?;
            let tokens_timelocked_target = record[3].parse()?;

            address_swap_split_map
                .map
                .entry(origin)
                .or_default()
                .destinations
                .push((destination_address, tokens_target, tokens_timelocked_target));
        }

        Ok(address_swap_split_map)
    }
}
