// Copyright 2020-2021 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use packable::Packable;

use crate::types::block::{
    Error, address::Address, payload::milestone::option::receipt::TailTransactionHash,
    protocol::ProtocolParameters,
};

/// Describes funds which were migrated from a legacy network.
#[derive(Clone, Debug, Eq, PartialEq, Packable)]
#[packable(unpack_visitor = ProtocolParameters)]
pub struct MigratedFundsEntry {
    tail_transaction_hash: TailTransactionHash,
    // The target address of the migrated funds.
    address: Address,
    // The migrated amount.
    #[packable(verify_with = verify_amount_packable)]
    amount: u64,
}

impl MigratedFundsEntry {
    /// Range of valid amounts for a [`MigratedFundsEntry`].
    pub const AMOUNT_MIN: u64 = 1_000_000;

    /// Creates a new [`MigratedFundsEntry`].
    pub fn new(
        tail_transaction_hash: TailTransactionHash,
        address: Address,
        amount: u64,
        token_supply: u64,
    ) -> Result<Self, Error> {
        verify_amount::<true>(&amount, &token_supply)?;

        Ok(Self {
            tail_transaction_hash,
            address,
            amount,
        })
    }

    #[inline(always)]
    /// Returns the tail transaction hash of a [`MigratedFundsEntry`].
    pub fn tail_transaction_hash(&self) -> &TailTransactionHash {
        &self.tail_transaction_hash
    }

    /// Returns the address of a [`MigratedFundsEntry`].
    #[inline(always)]
    pub fn address(&self) -> &Address {
        &self.address
    }

    /// Returns the amount of a [`MigratedFundsEntry`].
    #[inline(always)]
    pub fn amount(&self) -> u64 {
        self.amount
    }
}

fn verify_amount<const VERIFY: bool>(amount: &u64, token_supply: &u64) -> Result<(), Error> {
    if VERIFY && (*amount < MigratedFundsEntry::AMOUNT_MIN || amount > token_supply) {
        Err(Error::InvalidMigratedFundsEntryAmount(*amount))
    } else {
        Ok(())
    }
}

fn verify_amount_packable<const VERIFY: bool>(
    amount: &u64,
    protocol_parameters: &ProtocolParameters,
) -> Result<(), Error> {
    verify_amount::<VERIFY>(amount, &protocol_parameters.token_supply())
}

#[cfg(feature = "serde")]
pub(crate) mod dto {
    use alloc::string::String;

    use serde::{Deserialize, Serialize};

    use super::*;
    use crate::types::{
        TryFromDto, ValidationParams,
        block::{Error, address::dto::AddressDto},
    };

    #[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct MigratedFundsEntryDto {
        pub tail_transaction_hash: String,
        pub address: AddressDto,
        pub deposit: u64,
    }

    impl From<&MigratedFundsEntry> for MigratedFundsEntryDto {
        fn from(value: &MigratedFundsEntry) -> Self {
            Self {
                tail_transaction_hash: prefix_hex::encode(value.tail_transaction_hash().as_ref()),
                address: value.address().into(),
                deposit: value.amount(),
            }
        }
    }

    impl TryFromDto for MigratedFundsEntry {
        type Dto = MigratedFundsEntryDto;
        type Error = Error;

        fn try_from_dto_with_params_inner(
            dto: Self::Dto,
            params: ValidationParams<'_>,
        ) -> Result<Self, Self::Error> {
            let tail_transaction_hash = prefix_hex::decode(&dto.tail_transaction_hash)
                .map_err(|_| Error::InvalidField("tailTransactionHash"))?;

            Ok(if let Some(token_supply) = params.token_supply() {
                Self::new(
                    TailTransactionHash::new(tail_transaction_hash)?,
                    dto.address.try_into()?,
                    dto.deposit,
                    token_supply,
                )?
            } else {
                Self {
                    tail_transaction_hash: TailTransactionHash::new(tail_transaction_hash)?,
                    amount: dto.deposit,
                    address: dto.address.try_into()?,
                }
            })
        }
    }
}
