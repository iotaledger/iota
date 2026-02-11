// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use crate::block::{Error, address::Address, output::verify_output_amount};

/// Defines the amount of IOTAs used as storage deposit that have to be returned
/// to the return [`Address`].
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, packable::Packable)]
pub struct StorageDepositReturnUnlockCondition {
    // The [`Address`] to return the amount to.
    return_address: Address,
    // Amount of IOTA coins the consuming transaction should deposit to `return_address`.
    amount: u64,
}

impl StorageDepositReturnUnlockCondition {
    /// The [`UnlockCondition`](crate::block::output::UnlockCondition)
    /// kind of a [`StorageDepositReturnUnlockCondition`].
    pub const KIND: u8 = 1;

    /// Creates a new [`StorageDepositReturnUnlockCondition`].
    #[inline(always)]
    pub fn new(
        return_address: impl Into<Address>,
        amount: u64,
        token_supply: u64,
    ) -> Result<Self, Error> {
        verify_amount::<true>(&amount, &token_supply)?;

        Ok(Self {
            return_address: return_address.into(),
            amount,
        })
    }

    /// Returns the return address.
    #[inline(always)]
    pub fn return_address(&self) -> &Address {
        &self.return_address
    }

    /// Returns the amount.
    #[inline(always)]
    pub fn amount(&self) -> u64 {
        self.amount
    }
}

fn verify_amount<const VERIFY: bool>(amount: &u64, token_supply: &u64) -> Result<(), Error> {
    if VERIFY {
        verify_output_amount(amount, token_supply)
            .map_err(|_| Error::InvalidStorageDepositAmount(*amount))?;
    }

    Ok(())
}
