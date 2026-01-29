// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod alias_id;
mod chain_id;
mod foundry_id;
mod native_token;
mod nft_id;
mod output_id;
mod token_id;
mod token_scheme;
mod treasury;

///
pub mod alias;
///
pub mod basic;
///
pub mod feature;
///
pub mod foundry;
///
pub mod nft;
///
pub mod unlock_condition;

use core::ops::RangeInclusive;

use derive_more::From;
use packable::{
    Packable,
    error::{UnpackError, UnpackErrorExt},
    packer::Packer,
    unpacker::Unpacker,
};

pub(crate) use self::{
    alias::StateMetadataLength,
    feature::{MetadataFeatureLength, TagFeatureLength},
    native_token::NativeTokenCount,
    output_id::OutputIndex,
};
pub use self::{
    alias::{AliasOutput, AliasOutputBuilder, AliasTransition},
    alias_id::AliasId,
    basic::{BasicOutput, BasicOutputBuilder},
    chain_id::ChainId,
    feature::{Feature, Features},
    foundry::{FoundryOutput, FoundryOutputBuilder},
    foundry_id::FoundryId,
    native_token::{NativeToken, NativeTokens, NativeTokensBuilder},
    nft::{NftOutput, NftOutputBuilder},
    nft_id::NftId,
    output_id::OutputId,
    token_id::TokenId,
    token_scheme::{SimpleTokenScheme, TokenScheme},
    treasury::TreasuryOutput,
    unlock_condition::{UnlockCondition, UnlockConditions},
};
use crate::block::{Error, address::Address};

/// The maximum number of outputs of a transaction.
pub const OUTPUT_COUNT_MAX: u16 = 128;
/// The range of valid numbers of outputs of a transaction .
pub const OUTPUT_COUNT_RANGE: RangeInclusive<u16> = 1..=OUTPUT_COUNT_MAX; // [1..128]
/// The maximum index of outputs of a transaction.
pub const OUTPUT_INDEX_MAX: u16 = OUTPUT_COUNT_MAX - 1; // 127
/// The range of valid indices of outputs of a transaction .
pub const OUTPUT_INDEX_RANGE: RangeInclusive<u16> = 0..=OUTPUT_INDEX_MAX; // [0..127]
#[derive(Clone)]
pub(crate) enum OutputBuilderAmount {
    Amount(u64),
}

/// A generic output that can represent different types defining the deposit of
/// funds.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, From)]
pub enum Output {
    /// A treasury output.
    Treasury(TreasuryOutput),
    /// A basic output.
    Basic(BasicOutput),
    /// An alias output.
    Alias(AliasOutput),
    /// A foundry output.
    Foundry(FoundryOutput),
    /// An NFT output.
    Nft(NftOutput),
}

impl core::fmt::Debug for Output {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Treasury(output) => output.fmt(f),
            Self::Basic(output) => output.fmt(f),
            Self::Alias(output) => output.fmt(f),
            Self::Foundry(output) => output.fmt(f),
            Self::Nft(output) => output.fmt(f),
        }
    }
}

impl Output {
    /// Minimum amount for an output.
    pub const AMOUNT_MIN: u64 = 1;

    /// Return the output kind of an [`Output`].
    pub fn kind(&self) -> u8 {
        match self {
            Self::Treasury(_) => TreasuryOutput::KIND,
            Self::Basic(_) => BasicOutput::KIND,
            Self::Alias(_) => AliasOutput::KIND,
            Self::Foundry(_) => FoundryOutput::KIND,
            Self::Nft(_) => NftOutput::KIND,
        }
    }

    /// Returns the output kind of an [`Output`] as a string.
    pub fn kind_str(&self) -> &str {
        match self {
            Self::Alias(_) => "Alias",
            Self::Basic(_) => "Basic",
            Self::Foundry(_) => "Foundry",
            Self::Nft(_) => "Nft",
            Self::Treasury(_) => "Treasury",
        }
    }

    /// Returns the amount of an [`Output`].
    pub fn amount(&self) -> u64 {
        match self {
            Self::Treasury(output) => output.amount(),
            Self::Basic(output) => output.amount(),
            Self::Alias(output) => output.amount(),
            Self::Foundry(output) => output.amount(),
            Self::Nft(output) => output.amount(),
        }
    }

    /// Returns the native tokens of an [`Output`], if any.
    pub fn native_tokens(&self) -> Option<&NativeTokens> {
        match self {
            Self::Treasury(_) => None,
            Self::Basic(output) => Some(output.native_tokens()),
            Self::Alias(output) => Some(output.native_tokens()),
            Self::Foundry(output) => Some(output.native_tokens()),
            Self::Nft(output) => Some(output.native_tokens()),
        }
    }

    /// Returns the unlock conditions of an [`Output`], if any.
    pub fn unlock_conditions(&self) -> Option<&UnlockConditions> {
        match self {
            Self::Treasury(_) => None,
            Self::Basic(output) => Some(output.unlock_conditions()),
            Self::Alias(output) => Some(output.unlock_conditions()),
            Self::Foundry(output) => Some(output.unlock_conditions()),
            Self::Nft(output) => Some(output.unlock_conditions()),
        }
    }

    /// Returns the features of an [`Output`], if any.
    pub fn features(&self) -> Option<&Features> {
        match self {
            Self::Treasury(_) => None,
            Self::Basic(output) => Some(output.features()),
            Self::Alias(output) => Some(output.features()),
            Self::Foundry(output) => Some(output.features()),
            Self::Nft(output) => Some(output.features()),
        }
    }

    /// Returns the immutable features of an [`Output`], if any.
    pub fn immutable_features(&self) -> Option<&Features> {
        match self {
            Self::Treasury(_) => None,
            Self::Basic(_) => None,
            Self::Alias(output) => Some(output.immutable_features()),
            Self::Foundry(output) => Some(output.immutable_features()),
            Self::Nft(output) => Some(output.immutable_features()),
        }
    }

    /// Returns the chain identifier of an [`Output`], if any.
    pub fn chain_id(&self) -> Option<ChainId> {
        match self {
            Self::Treasury(_) => None,
            Self::Basic(_) => None,
            Self::Alias(output) => Some(output.chain_id()),
            Self::Foundry(output) => Some(output.chain_id()),
            Self::Nft(output) => Some(output.chain_id()),
        }
    }

    /// Checks whether the output is a [`TreasuryOutput`].
    pub fn is_treasury(&self) -> bool {
        matches!(self, Self::Treasury(_))
    }

    /// Gets the output as an actual [`TreasuryOutput`].
    /// PANIC: do not call on a non-treasury output.
    pub fn as_treasury(&self) -> &TreasuryOutput {
        if let Self::Treasury(output) = self {
            output
        } else {
            panic!("as_treasury called on a non-treasury output");
        }
    }

    /// Checks whether the output is a [`BasicOutput`].
    pub fn is_basic(&self) -> bool {
        matches!(self, Self::Basic(_))
    }

    /// Gets the output as an actual [`BasicOutput`].
    /// PANIC: do not call on a non-basic output.
    pub fn as_basic(&self) -> &BasicOutput {
        if let Self::Basic(output) = self {
            output
        } else {
            panic!("as_basic called on a non-basic output");
        }
    }

    /// Checks whether the output is an [`AliasOutput`].
    pub fn is_alias(&self) -> bool {
        matches!(self, Self::Alias(_))
    }

    /// Gets the output as an actual [`AliasOutput`].
    /// PANIC: do not call on a non-alias output.
    pub fn as_alias(&self) -> &AliasOutput {
        if let Self::Alias(output) = self {
            output
        } else {
            panic!("as_alias called on a non-alias output");
        }
    }

    /// Checks whether the output is a [`FoundryOutput`].
    pub fn is_foundry(&self) -> bool {
        matches!(self, Self::Foundry(_))
    }

    /// Gets the output as an actual [`FoundryOutput`].
    /// PANIC: do not call on a non-foundry output.
    pub fn as_foundry(&self) -> &FoundryOutput {
        if let Self::Foundry(output) = self {
            output
        } else {
            panic!("as_foundry called on a non-foundry output");
        }
    }

    /// Checks whether the output is an [`NftOutput`].
    pub fn is_nft(&self) -> bool {
        matches!(self, Self::Nft(_))
    }

    /// Gets the output as an actual [`NftOutput`].
    /// PANIC: do not call on a non-nft output.
    pub fn as_nft(&self) -> &NftOutput {
        if let Self::Nft(output) = self {
            output
        } else {
            panic!("as_nft called on a non-nft output");
        }
    }

    /// Returns the address that is required to unlock this [`Output`] and the
    /// alias or nft address that gets unlocked by it, if it's an alias or
    /// nft. If no `alias_transition` has been provided, assumes a state
    /// transition.
    pub fn required_and_unlocked_address(
        &self,
        current_time: u32,
        output_id: &OutputId,
        alias_transition: Option<AliasTransition>,
    ) -> Result<(Address, Option<Address>), Error> {
        match self {
            Self::Alias(output) => {
                if alias_transition.unwrap_or(AliasTransition::State) == AliasTransition::State {
                    // Alias address is only unlocked if it's a state transition
                    Ok((
                        *output.state_controller_address(),
                        Some(Address::Alias(output.alias_address(output_id))),
                    ))
                } else {
                    Ok((*output.governor_address(), None))
                }
            }
            Self::Basic(output) => Ok((
                *output
                    .unlock_conditions()
                    .locked_address(output.address(), current_time),
                None,
            )),
            Self::Nft(output) => Ok((
                *output
                    .unlock_conditions()
                    .locked_address(output.address(), current_time),
                Some(Address::Nft(output.nft_address(output_id))),
            )),
            Self::Foundry(output) => Ok((Address::Alias(*output.alias_address()), None)),
            Self::Treasury(_) => Err(Error::UnsupportedOutputKind(TreasuryOutput::KIND)),
        }
    }
}

impl Packable for Output {
    type UnpackVisitor = ();
    type UnpackError = Error;

    fn pack<P: Packer>(&self, packer: &mut P) -> Result<(), P::Error> {
        match self {
            Self::Treasury(output) => {
                TreasuryOutput::KIND.pack(packer)?;
                output.pack(packer)
            }
            Self::Basic(output) => {
                BasicOutput::KIND.pack(packer)?;
                output.pack(packer)
            }
            Self::Alias(output) => {
                AliasOutput::KIND.pack(packer)?;
                output.pack(packer)
            }
            Self::Foundry(output) => {
                FoundryOutput::KIND.pack(packer)?;
                output.pack(packer)
            }
            Self::Nft(output) => {
                NftOutput::KIND.pack(packer)?;
                output.pack(packer)
            }
        }
    }

    fn unpack<U: Unpacker, const VERIFY: bool>(
        unpacker: &mut U,
        visitor: &Self::UnpackVisitor,
    ) -> Result<Self, UnpackError<Self::UnpackError, U::Error>> {
        let kind = u8::unpack::<_, VERIFY>(unpacker, visitor).coerce()?;

        match kind {
            TreasuryOutput::KIND => Ok(Self::Treasury(
                TreasuryOutput::unpack::<_, VERIFY>(unpacker, visitor).coerce()?,
            )),
            BasicOutput::KIND => Ok(Self::Basic(
                BasicOutput::unpack::<_, VERIFY>(unpacker, visitor).coerce()?,
            )),
            AliasOutput::KIND => Ok(Self::Alias(
                AliasOutput::unpack::<_, VERIFY>(unpacker, visitor).coerce()?,
            )),
            FoundryOutput::KIND => Ok(Self::Foundry(
                FoundryOutput::unpack::<_, VERIFY>(unpacker, visitor).coerce()?,
            )),
            NftOutput::KIND => Ok(Self::Nft(
                NftOutput::unpack::<_, VERIFY>(unpacker, visitor).coerce()?,
            )),
            _ => Err(UnpackError::Packable(Error::InvalidOutputKind(kind))),
        }
    }
}

pub(crate) fn verify_output_amount(amount: &u64, token_supply: &u64) -> Result<(), Error> {
    if *amount < Output::AMOUNT_MIN || amount > token_supply {
        Err(Error::InvalidOutputAmount(*amount))
    } else {
        Ok(())
    }
}
