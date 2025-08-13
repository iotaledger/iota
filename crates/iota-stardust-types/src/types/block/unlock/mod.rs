// Copyright 2020-2021 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod alias;
mod nft;
mod reference;
mod signature;

use alloc::boxed::Box;
use core::ops::RangeInclusive;

use derive_more::{Deref, From};
use hashbrown::HashSet;
use packable::{Packable, bounded::BoundedU16, prefix::BoxedSlicePrefix};

pub use self::{
    alias::AliasUnlock, nft::NftUnlock, reference::ReferenceUnlock, signature::SignatureUnlock,
};
use crate::types::block::{
    Error,
    input::{INPUT_COUNT_MAX, INPUT_COUNT_RANGE, INPUT_INDEX_MAX, INPUT_INDEX_RANGE},
};

/// The maximum number of unlocks of a transaction.
pub const UNLOCK_COUNT_MAX: u16 = INPUT_COUNT_MAX; // 128
/// The range of valid numbers of unlocks of a transaction.
pub const UNLOCK_COUNT_RANGE: RangeInclusive<u16> = INPUT_COUNT_RANGE; // [1..128]
/// The maximum index of unlocks of a transaction.
pub const UNLOCK_INDEX_MAX: u16 = INPUT_INDEX_MAX; // 127
/// The range of valid indices of unlocks of a transaction.
pub const UNLOCK_INDEX_RANGE: RangeInclusive<u16> = INPUT_INDEX_RANGE; // [0..127]

pub(crate) type UnlockIndex =
    BoundedU16<{ *UNLOCK_INDEX_RANGE.start() }, { *UNLOCK_INDEX_RANGE.end() }>;

/// Defines the mechanism by which a transaction input is authorized to be
/// consumed.
#[derive(Clone, Eq, PartialEq, Hash, From, Packable)]
#[packable(unpack_error = Error)]
#[packable(tag_type = u8, with_error = Error::InvalidUnlockKind)]
pub enum Unlock {
    /// A signature unlock.
    #[packable(tag = SignatureUnlock::KIND)]
    Signature(SignatureUnlock),
    /// A reference unlock.
    #[packable(tag = ReferenceUnlock::KIND)]
    Reference(ReferenceUnlock),
    /// An alias unlock.
    #[packable(tag = AliasUnlock::KIND)]
    Alias(AliasUnlock),
    /// An NFT unlock.
    #[packable(tag = NftUnlock::KIND)]
    Nft(NftUnlock),
}

impl core::fmt::Debug for Unlock {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Signature(unlock) => unlock.fmt(f),
            Self::Reference(unlock) => unlock.fmt(f),
            Self::Alias(unlock) => unlock.fmt(f),
            Self::Nft(unlock) => unlock.fmt(f),
        }
    }
}

impl Unlock {
    /// Returns the unlock kind of an [`Unlock`].
    pub fn kind(&self) -> u8 {
        match self {
            Self::Signature(_) => SignatureUnlock::KIND,
            Self::Reference(_) => ReferenceUnlock::KIND,
            Self::Alias(_) => AliasUnlock::KIND,
            Self::Nft(_) => NftUnlock::KIND,
        }
    }
}

pub(crate) type UnlockCount =
    BoundedU16<{ *UNLOCK_COUNT_RANGE.start() }, { *UNLOCK_COUNT_RANGE.end() }>;

/// A collection of unlocks.
#[derive(Clone, Debug, Eq, PartialEq, Deref, Packable)]
#[packable(unpack_error = Error, with = |e| e.unwrap_item_err_or_else(|p| Error::InvalidUnlockCount(p.into())))]
pub struct Unlocks(#[packable(verify_with = verify_unlocks)] BoxedSlicePrefix<Unlock, UnlockCount>);

impl Unlocks {
    /// Creates a new [`Unlocks`].
    pub fn new(unlocks: impl Into<Box<[Unlock]>>) -> Result<Self, Error> {
        let unlocks: BoxedSlicePrefix<Unlock, UnlockCount> = unlocks
            .into()
            .try_into()
            .map_err(Error::InvalidUnlockCount)?;

        verify_unlocks::<true>(&unlocks, &())?;

        Ok(Self(unlocks))
    }

    /// Gets an [`Unlock`] from an [`Unlocks`].
    /// Returns the referenced unlock if the requested unlock was a reference.
    pub fn get(&self, index: usize) -> Option<&Unlock> {
        match self.0.get(index) {
            Some(Unlock::Reference(reference)) => self.0.get(reference.index() as usize),
            Some(unlock) => Some(unlock),
            None => None,
        }
    }
}

fn verify_unlocks<const VERIFY: bool>(unlocks: &[Unlock], _: &()) -> Result<(), Error> {
    if VERIFY {
        let mut seen_signatures = HashSet::new();

        for (index, unlock) in (0u16..).zip(unlocks.iter()) {
            match unlock {
                Unlock::Signature(signature) => {
                    if !seen_signatures.insert(signature) {
                        return Err(Error::DuplicateSignatureUnlock(index));
                    }
                }
                Unlock::Reference(reference) => {
                    if index == 0
                        || reference.index() >= index
                        || !matches!(unlocks[reference.index() as usize], Unlock::Signature(_))
                    {
                        return Err(Error::InvalidUnlockReference(index));
                    }
                }
                Unlock::Alias(alias) => {
                    if index == 0 || alias.index() >= index {
                        return Err(Error::InvalidUnlockAlias(index));
                    }
                }
                Unlock::Nft(nft) => {
                    if index == 0 || nft.index() >= index {
                        return Err(Error::InvalidUnlockNft(index));
                    }
                }
            }
        }
    }

    Ok(())
}

#[cfg(feature = "serde")]
pub mod dto {
    use alloc::format;

    use serde::{Deserialize, Serialize, Serializer};
    use serde_json::Value;

    use super::*;
    pub use super::{
        alias::dto::AliasUnlockDto, nft::dto::NftUnlockDto, reference::dto::ReferenceUnlockDto,
        signature::dto::SignatureUnlockDto,
    };
    use crate::types::block::{
        Error,
        signature::{Ed25519Signature, Signature, dto::SignatureDto},
    };

    /// Describes all the different unlock types.
    #[derive(Clone, Debug, Eq, PartialEq, From)]
    pub enum UnlockDto {
        Signature(SignatureUnlockDto),
        Reference(ReferenceUnlockDto),
        Alias(AliasUnlockDto),
        Nft(NftUnlockDto),
    }

    impl From<&Unlock> for UnlockDto {
        fn from(value: &Unlock) -> Self {
            match value {
                Unlock::Signature(signature) => match signature.signature() {
                    Signature::Ed25519(ed) => Self::Signature(SignatureUnlockDto {
                        kind: SignatureUnlock::KIND,
                        signature: SignatureDto::Ed25519(Box::new(ed.as_ref().into())),
                    }),
                },
                Unlock::Reference(r) => Self::Reference(ReferenceUnlockDto {
                    kind: ReferenceUnlock::KIND,
                    index: r.index(),
                }),
                Unlock::Alias(a) => Self::Alias(AliasUnlockDto {
                    kind: AliasUnlock::KIND,
                    index: a.index(),
                }),
                Unlock::Nft(n) => Self::Nft(NftUnlockDto {
                    kind: NftUnlock::KIND,
                    index: n.index(),
                }),
            }
        }
    }

    impl TryFrom<UnlockDto> for Unlock {
        type Error = Error;

        fn try_from(value: UnlockDto) -> Result<Self, Self::Error> {
            match value {
                UnlockDto::Signature(s) => match s.signature {
                    SignatureDto::Ed25519(ed) => Ok(Self::Signature(SignatureUnlock::from(
                        Signature::Ed25519(Ed25519Signature::try_from(*ed)?.into()),
                    ))),
                },
                UnlockDto::Reference(r) => Ok(Self::Reference(ReferenceUnlock::new(r.index)?)),
                UnlockDto::Alias(a) => Ok(Self::Alias(AliasUnlock::new(a.index)?)),
                UnlockDto::Nft(n) => Ok(Self::Nft(NftUnlock::new(n.index)?)),
            }
        }
    }

    impl<'de> Deserialize<'de> for UnlockDto {
        fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            let value = Value::deserialize(d)?;
            Ok(
                match value
                    .get("type")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| serde::de::Error::custom("invalid unlock type"))?
                    as u8
                {
                    SignatureUnlock::KIND => {
                        Self::Signature(SignatureUnlockDto::deserialize(value).map_err(|e| {
                            serde::de::Error::custom(format!(
                                "cannot deserialize signature unlock: {e}"
                            ))
                        })?)
                    }
                    ReferenceUnlock::KIND => {
                        Self::Reference(ReferenceUnlockDto::deserialize(value).map_err(|e| {
                            serde::de::Error::custom(format!(
                                "cannot deserialize reference unlock: {e}"
                            ))
                        })?)
                    }
                    AliasUnlock::KIND => {
                        Self::Alias(AliasUnlockDto::deserialize(value).map_err(|e| {
                            serde::de::Error::custom(format!(
                                "cannot deserialize alias unlock: {e}"
                            ))
                        })?)
                    }
                    NftUnlock::KIND => {
                        Self::Nft(NftUnlockDto::deserialize(value).map_err(|e| {
                            serde::de::Error::custom(format!("cannot deserialize NFT unlock: {e}"))
                        })?)
                    }
                    _ => return Err(serde::de::Error::custom("invalid unlock type")),
                },
            )
        }
    }

    impl Serialize for UnlockDto {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            #[derive(Serialize)]
            #[serde(untagged)]
            enum UnlockDto_<'a> {
                T1(&'a SignatureUnlockDto),
                T2(&'a ReferenceUnlockDto),
                T3(&'a AliasUnlockDto),
                T4(&'a NftUnlockDto),
            }
            #[derive(Serialize)]
            struct TypedUnlock<'a> {
                #[serde(flatten)]
                unlock: UnlockDto_<'a>,
            }
            let unlock = match self {
                Self::Signature(o) => TypedUnlock {
                    unlock: UnlockDto_::T1(o),
                },
                Self::Reference(o) => TypedUnlock {
                    unlock: UnlockDto_::T2(o),
                },
                Self::Alias(o) => TypedUnlock {
                    unlock: UnlockDto_::T3(o),
                },
                Self::Nft(o) => TypedUnlock {
                    unlock: UnlockDto_::T4(o),
                },
            };
            unlock.serialize(serializer)
        }
    }
}
