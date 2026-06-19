// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_ext::types::{Address, Owner};
use iota_stardust_types::block::address::Address as StardustAddress;

/// Converts a ["Stardust" `Address`](StardustAddress) to an [`Address`].
///
/// This is intended as the only conversion function to go from Stardust to IOTA
/// addresses, so there is only one place to potentially update it if we decide
/// to change it later.
pub fn stardust_to_iota_address(
    stardust_address: impl Into<StardustAddress>,
) -> anyhow::Result<Address> {
    Ok(stardust_address.into().to_string().parse()?)
}

/// Converts a ["Stardust" `Address`](StardustAddress) to an [`Address`] and
/// then wraps it into an [`Owner`] which is either address- or object-owned
/// depending on the stardust address.
pub fn stardust_to_iota_address_owner(
    stardust_address: impl Into<StardustAddress>,
) -> anyhow::Result<Owner> {
    stardust_to_iota_address(stardust_address.into()).map(Owner::Address)
}
