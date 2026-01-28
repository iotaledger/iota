// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use derive_more::From;

use crate::block::address::Address;

/// Defines the Address that owns this output, that is, it can unlock it with
/// the proper Unlock in a transaction.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, From, packable::Packable)]
pub struct AddressUnlockCondition(Address);

impl AddressUnlockCondition {
    /// The [`UnlockCondition`](crate::block::output::UnlockCondition)
    /// kind of an [`AddressUnlockCondition`].
    pub const KIND: u8 = 0;

    /// Creates a new [`AddressUnlockCondition`].
    #[inline(always)]
    pub fn new(address: impl Into<Address>) -> Self {
        Self(address.into())
    }

    /// Returns the address of a [`AddressUnlockCondition`].
    #[inline(always)]
    pub fn address(&self) -> &Address {
        &self.0
    }
}
