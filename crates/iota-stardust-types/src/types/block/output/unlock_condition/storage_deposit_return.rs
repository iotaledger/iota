// Copyright 2021 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use crate::types::block::{
    Error, address::Address, output::verify_output_amount, protocol::ProtocolParameters,
};

/// Defines the amount of IOTAs used as storage deposit that have to be returned
/// to the return [`Address`].
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, packable::Packable)]
#[packable(unpack_visitor = ProtocolParameters)]
pub struct StorageDepositReturnUnlockCondition {
    // The [`Address`] to return the amount to.
    return_address: Address,
    // Amount of IOTA coins the consuming transaction should deposit to `return_address`.
    #[packable(verify_with = verify_amount_packable)]
    amount: u64,
}

impl StorageDepositReturnUnlockCondition {
    /// The [`UnlockCondition`](crate::types::block::output::UnlockCondition)
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

fn verify_amount_packable<const VERIFY: bool>(
    amount: &u64,
    protocol_parameters: &ProtocolParameters,
) -> Result<(), Error> {
    verify_amount::<VERIFY>(amount, &protocol_parameters.token_supply())
}

#[cfg(feature = "serde")]
pub(crate) mod dto {
    use alloc::string::{String, ToString};

    use serde::{Deserialize, Serialize};

    use super::*;
    use crate::types::{
        TryFromDto, ValidationParams,
        block::{Error, address::dto::AddressDto},
    };

    #[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct StorageDepositReturnUnlockConditionDto {
        #[serde(rename = "type")]
        pub kind: u8,
        pub return_address: AddressDto,
        pub amount: String,
    }

    impl From<&StorageDepositReturnUnlockCondition> for StorageDepositReturnUnlockConditionDto {
        fn from(value: &StorageDepositReturnUnlockCondition) -> Self {
            Self {
                kind: StorageDepositReturnUnlockCondition::KIND,
                return_address: AddressDto::from(value.return_address()),
                amount: value.amount().to_string(),
            }
        }
    }

    impl TryFromDto for StorageDepositReturnUnlockCondition {
        type Dto = StorageDepositReturnUnlockConditionDto;
        type Error = Error;

        fn try_from_dto_with_params_inner(
            dto: Self::Dto,
            params: ValidationParams<'_>,
        ) -> Result<Self, Self::Error> {
            Ok(if let Some(token_supply) = params.token_supply() {
                Self::new(
                    Address::try_from(dto.return_address)?,
                    dto.amount
                        .parse::<u64>()
                        .map_err(|_| Error::InvalidField("amount"))?,
                    token_supply,
                )?
            } else {
                Self {
                    return_address: Address::try_from(dto.return_address)?,
                    amount: dto
                        .amount
                        .parse::<u64>()
                        .map_err(|_| Error::InvalidField("amount"))?,
                }
            })
        }
    }
}
