// Copyright (c) The Diem Core Contributors
// Copyright (c) The Move Contributors
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub use iota_sdk::types::{
    Address as AccountAddress, AddressParseError as AccountAddressParseError,
};

use crate::gas_algebra::AbstractMemorySize;

/// TODO (ade): use macro to enforce determinism
pub fn address_abstract_size_for_gas_metering() -> AbstractMemorySize {
    AbstractMemorySize::new(AccountAddress::LENGTH as u64)
}
