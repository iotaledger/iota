// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use derive_more::From;

use crate::block::address::Address;

/// Defines the Governor Address that owns this output, that is, it can unlock
/// it with the proper Unlock in a transaction that governance transitions the
/// alias output.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, From, packable::Packable)]
pub struct GovernorAddressUnlockCondition(Address);

impl GovernorAddressUnlockCondition {
    /// The [`UnlockCondition`](crate::block::output::UnlockCondition)
    /// kind of an [`GovernorAddressUnlockCondition`].
    pub const KIND: u8 = 5;

    /// Creates a new [`GovernorAddressUnlockCondition`].
    #[inline(always)]
    pub fn new(address: impl Into<Address>) -> Self {
        Self(address.into())
    }

    /// Returns the address of a [`GovernorAddressUnlockCondition`].
    #[inline(always)]
    pub fn address(&self) -> &Address {
        &self.0
    }
}
