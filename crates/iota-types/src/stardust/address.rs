// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_stardust_sdk::types::block::address::Address;

use crate::{address_swap_map::AddressSwapMap, base_types::IotaAddress, object::Owner};

/// Converts a ["Stardust" `Address`](Address) to a [`IotaAddress`].
///
/// This is intended as the only conversion function to go from Stardust to Iota
/// addresses, so there is only one place to potentially update it if we decide
/// to change it later.
pub fn stardust_to_iota_address(
    stardust_address: impl Into<Address>,
) -> anyhow::Result<IotaAddress> {
    stardust_address.into().to_string().parse()
}

/// Converts a ["Stardust" `Address`](Address) to a [`IotaAddress`] and then
/// wraps it into an [`Owner`] which is either address- or object-owned
/// depending on the stardust address.
pub fn stardust_to_iota_address_owner(
    stardust_address: impl Into<Address>,
) -> anyhow::Result<Owner> {
    stardust_to_iota_address(stardust_address.into()).map(Owner::AddressOwner)
}

pub fn stardust_to_iota_address_owner_maybe_swap(
    stardust_address: impl Into<Address>,
    address_swap_map: &mut AddressSwapMap,
) -> anyhow::Result<Owner> {
    let mut address = stardust_to_iota_address(stardust_address)?;
    if let Some(addr) = address_swap_map.get_destination_address(address) {
        address = *addr;
    }
    Ok(Owner::AddressOwner(address))
}

pub fn stardust_to_iota_address_maybe_swap(
    stardust_address: impl Into<Address>,
    address_swap_map: &mut AddressSwapMap,
) -> anyhow::Result<IotaAddress> {
    let mut address: IotaAddress = stardust_to_iota_address(stardust_address)?;
    if let Some(addr) = address_swap_map.get_destination_address(address) {
        address = *addr;
    }
    Ok(address)
}
