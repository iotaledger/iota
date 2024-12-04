// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_stardust_sdk::types::block::address::Address;

use super::address_swap_map::AddressSwapMap;
use crate::{base_types::IotaAddress, object::Owner};

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

/// Converts a ["Stardust" `Address`](Address) to an [`Owner`] by first
/// converting it to an [`IotaAddress`] and then checking against the provided
/// `AddressSwapMap` for potential address substitutions.
///
/// If the address exists in the `AddressSwapMap`, it is swapped with the mapped
/// destination address before being wrapped into an [`Owner`].
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

/// Converts a ["Stardust" `Address`](Address) to an [`IotaAddress`] and checks
/// against the provided `AddressSwapMap` for potential address substitutions.
///
/// If the address exists in the `AddressSwapMap`, it is swapped with the mapped
/// destination address before being returned as an [`IotaAddress`].
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
