// Copyright 2020-2021 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use crate::types::block::{Error, protocol::ProtocolParameters};

/// [`TreasuryOutput`] is an output which holds the treasury of a network.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, packable::Packable)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[packable(unpack_error = Error)]
#[packable(unpack_visitor = ProtocolParameters)]
pub struct TreasuryOutput {
    #[packable(verify_with = verify_amount_packable)]
    amount: u64,
}

impl TreasuryOutput {
    /// The [`Output`](crate::types::block::output::Output) kind of a
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

fn verify_amount_packable<const VERIFY: bool>(
    amount: &u64,
    protocol_parameters: &ProtocolParameters,
) -> Result<(), Error> {
    if VERIFY {
        verify_amount(amount, &protocol_parameters.token_supply())?;
    }
    Ok(())
}

#[cfg(feature = "serde")]
pub(crate) mod dto {
    use alloc::string::{String, ToString};

    use serde::{Deserialize, Serialize};

    use super::*;
    use crate::types::{TryFromDto, ValidationParams, block::Error};

    /// Describes a treasury output.
    #[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
    pub struct TreasuryOutputDto {
        #[serde(rename = "type")]
        pub kind: u8,
        pub amount: String,
    }

    impl From<&TreasuryOutput> for TreasuryOutputDto {
        fn from(value: &TreasuryOutput) -> Self {
            Self {
                kind: TreasuryOutput::KIND,
                amount: value.amount().to_string(),
            }
        }
    }

    impl TryFromDto for TreasuryOutput {
        type Dto = TreasuryOutputDto;
        type Error = Error;

        fn try_from_dto_with_params_inner(
            dto: Self::Dto,
            params: ValidationParams<'_>,
        ) -> Result<Self, Self::Error> {
            Ok(if let Some(token_supply) = params.token_supply() {
                Self::new(
                    dto.amount
                        .parse::<u64>()
                        .map_err(|_| Error::InvalidField("amount"))?,
                    token_supply,
                )?
            } else {
                Self {
                    amount: dto
                        .amount
                        .parse::<u64>()
                        .map_err(|_| Error::InvalidField("amount"))?,
                }
            })
        }
    }
}
