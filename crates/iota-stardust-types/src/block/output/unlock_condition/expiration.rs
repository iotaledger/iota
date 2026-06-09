// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use derive_more::From;

use crate::block::{Error, address::Address};

/// Defines a unix time until which only Address, defined in Address Unlock
/// Condition, is allowed to unlock the output. After or at the unix time, only
/// Return Address can unlock it.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, From, packable::Packable)]
pub struct ExpirationUnlockCondition {
    // The address that can unlock the expired output.
    return_address: Address,
    // Before this unix time, seconds since unix epoch,
    // [`AddressUnlockCondition`](crate::unlock_condition::AddressUnlockCondition) is
    // allowed to unlock the output. After that, only the return
    // [`Address`](crate::address::Address) can.
    #[packable(verify_with = verify_timestamp)]
    timestamp: u32,
}

impl ExpirationUnlockCondition {
    /// The [`UnlockCondition`](crate::block::output::UnlockCondition)
    /// kind of an [`ExpirationUnlockCondition`].
    pub const KIND: u8 = 3;

    /// Creates a new [`ExpirationUnlockCondition`].
    #[inline(always)]
    pub fn new(return_address: impl Into<Address>, timestamp: u32) -> Result<Self, Error> {
        verify_timestamp::<true>(&timestamp, &())?;

        Ok(Self {
            return_address: return_address.into(),
            timestamp,
        })
    }

    /// Returns the return address of a [`ExpirationUnlockCondition`].
    #[inline(always)]
    pub fn return_address(&self) -> &Address {
        &self.return_address
    }

    /// Returns the timestamp of a [`ExpirationUnlockCondition`].
    #[inline(always)]
    pub fn timestamp(&self) -> u32 {
        self.timestamp
    }

    /// Returns the return address if the condition has expired.
    pub fn return_address_expired(&self, timestamp: u32) -> Option<&Address> {
        if timestamp >= self.timestamp() {
            Some(&self.return_address)
        } else {
            None
        }
    }
}

#[inline]
fn verify_timestamp<const VERIFY: bool>(timestamp: &u32, _: &()) -> Result<(), Error> {
    if VERIFY && *timestamp == 0 {
        Err(Error::ExpirationUnlockConditionZero)
    } else {
        Ok(())
    }
}
