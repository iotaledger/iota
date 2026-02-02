// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use crate::block::Error;

/// [`TreasuryOutput`] is an output which holds the treasury of a network.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, packable::Packable)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[packable(unpack_error = Error)]
pub struct TreasuryOutput {
    amount: u64,
}

impl TreasuryOutput {
    /// The [`Output`](crate::block::output::Output) kind of a
    /// [`TreasuryOutput`].
    pub const KIND: u8 = 2;

    /// Creates a new [`TreasuryOutput`].
    pub fn new(amount: u64, token_supply: u64) -> Result<Self, Error> {
        verify_amount(&amount, &token_supply)?;

        Ok(Self { amount })
    }

    /// Returns the amount of a [`TreasuryOutput`].
    #[inline(always)]
    pub fn amount(&self) -> u64 {
        self.amount
    }
}

fn verify_amount(amount: &u64, token_supply: &u64) -> Result<(), Error> {
    if amount > token_supply {
        Err(Error::InvalidTreasuryOutputAmount(*amount))
    } else {
        Ok(())
    }
}
