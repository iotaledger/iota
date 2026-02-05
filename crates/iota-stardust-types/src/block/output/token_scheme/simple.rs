// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use packable::{
    Packable,
    error::{UnpackError, UnpackErrorExt},
    packer::Packer,
    unpacker::Unpacker,
};
use primitive_types::U256;

use crate::block::Error;

///
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct SimpleTokenScheme {
    // Amount of tokens minted by a foundry.
    minted_tokens: U256,
    // Amount of tokens melted by a foundry.
    melted_tokens: U256,
    // Maximum supply of tokens controlled by a foundry.
    maximum_supply: U256,
}

impl SimpleTokenScheme {
    /// The [`TokenScheme`](crate::block::output::TokenScheme) kind of a
    /// [`SimpleTokenScheme`].
    pub const KIND: u8 = 0;

    /// Creates a new [`SimpleTokenScheme`].
    #[inline(always)]
    pub fn new(
        minted_tokens: impl Into<U256>,
        melted_tokens: impl Into<U256>,
        maximum_supply: impl Into<U256>,
    ) -> Result<Self, Error> {
        let minted_tokens = minted_tokens.into();
        let melted_tokens = melted_tokens.into();
        let maximum_supply = maximum_supply.into();

        verify_supply(&minted_tokens, &melted_tokens, &maximum_supply)?;

        Ok(Self {
            minted_tokens,
            melted_tokens,
            maximum_supply,
        })
    }

    /// Returns the number of minted tokens of the [`SimpleTokenScheme`].
    #[inline(always)]
    pub fn minted_tokens(&self) -> U256 {
        self.minted_tokens
    }

    /// Returns the number of melted tokens of the [`SimpleTokenScheme`].
    #[inline(always)]
    pub fn melted_tokens(&self) -> U256 {
        self.melted_tokens
    }

    /// Returns the maximum supply of the [`SimpleTokenScheme`].
    #[inline(always)]
    pub fn maximum_supply(&self) -> U256 {
        self.maximum_supply
    }

    /// Returns the circulating supply of the [`SimpleTokenScheme`].
    #[inline(always)]
    pub fn circulating_supply(&self) -> U256 {
        self.minted_tokens - self.melted_tokens
    }
}

impl Packable for SimpleTokenScheme {
    type UnpackError = Error;
    type UnpackVisitor = ();

    fn pack<P: Packer>(&self, packer: &mut P) -> Result<(), P::Error> {
        self.minted_tokens.pack(packer)?;
        self.melted_tokens.pack(packer)?;
        self.maximum_supply.pack(packer)?;

        Ok(())
    }

    fn unpack<U: Unpacker, const VERIFY: bool>(
        unpacker: &mut U,
        visitor: &Self::UnpackVisitor,
    ) -> Result<Self, UnpackError<Self::UnpackError, U::Error>> {
        let minted_tokens = U256::unpack::<_, VERIFY>(unpacker, visitor).coerce()?;
        let melted_tokens = U256::unpack::<_, VERIFY>(unpacker, visitor).coerce()?;
        let maximum_supply = U256::unpack::<_, VERIFY>(unpacker, visitor).coerce()?;

        if VERIFY {
            verify_supply(&minted_tokens, &melted_tokens, &maximum_supply)
                .map_err(UnpackError::Packable)?;
        }

        Ok(Self {
            minted_tokens,
            melted_tokens,
            maximum_supply,
        })
    }
}

#[inline]
fn verify_supply(
    minted_tokens: &U256,
    melted_tokens: &U256,
    maximum_supply: &U256,
) -> Result<(), Error> {
    if maximum_supply.is_zero()
        || melted_tokens > minted_tokens
        || minted_tokens - melted_tokens > *maximum_supply
    {
        return Err(Error::InvalidFoundryOutputSupply {
            minted: *minted_tokens,
            melted: *melted_tokens,
            max: *maximum_supply,
        });
    }

    Ok(())
}
