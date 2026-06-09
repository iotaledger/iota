// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use derive_more::From;

use crate::block::Error;

/// Defines a unix timestamp until which the output can not be unlocked.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, From, packable::Packable)]
#[packable(unpack_error = Error)]
pub struct TimelockUnlockCondition(#[packable(verify_with = verify_timestamp)] u32);

impl TimelockUnlockCondition {
    /// The [`UnlockCondition`](crate::block::output::UnlockCondition)
    /// kind of a [`TimelockUnlockCondition`].
    pub const KIND: u8 = 2;

    /// Creates a new [`TimelockUnlockCondition`].
    #[inline(always)]
    pub fn new(timestamp: u32) -> Result<Self, Error> {
        verify_timestamp::<true>(&timestamp, &())?;

        Ok(Self(timestamp))
    }

    /// Returns the timestamp of a [`TimelockUnlockCondition`].
    #[inline(always)]
    pub fn timestamp(&self) -> u32 {
        self.0
    }
}

#[inline]
fn verify_timestamp<const VERIFY: bool>(timestamp: &u32, _: &()) -> Result<(), Error> {
    if VERIFY && *timestamp == 0 {
        Err(Error::TimelockUnlockConditionZero)
    } else {
        Ok(())
    }
}
