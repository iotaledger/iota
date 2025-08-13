// Copyright 2021-2022 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use derive_more::From;

use crate::types::block::address::Address;

/// Defines the Address that owns this output, that is, it can unlock it with
/// the proper Unlock in a transaction.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, From, packable::Packable)]
pub struct AddressUnlockCondition(Address);

impl AddressUnlockCondition {
    /// The [`UnlockCondition`](crate::types::block::output::UnlockCondition)
    /// kind of an [`AddressUnlockCondition`].
    pub const KIND: u8 = 0;

    /// Creates a new [`AddressUnlockCondition`].
    #[inline(always)]
    pub fn new(address: impl Into<Address>) -> Self {
        Self(address.into())
    }

    /// Returns the address of a [`AddressUnlockCondition`].
    #[inline(always)]
    pub fn address(&self) -> &Address {
        &self.0
    }
}

#[cfg(feature = "serde")]
pub(crate) mod dto {
    use serde::{Deserialize, Serialize};

    use super::*;
    use crate::types::block::{Error, address::dto::AddressDto};

    #[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
    pub struct AddressUnlockConditionDto {
        #[serde(rename = "type")]
        pub kind: u8,
        pub address: AddressDto,
    }

    impl From<&AddressUnlockCondition> for AddressUnlockConditionDto {
        fn from(value: &AddressUnlockCondition) -> Self {
            Self {
                kind: AddressUnlockCondition::KIND,
                address: value.address().into(),
            }
        }
    }

    impl TryFrom<AddressUnlockConditionDto> for AddressUnlockCondition {
        type Error = Error;

        fn try_from(value: AddressUnlockConditionDto) -> Result<Self, Error> {
            Ok(Self::new(Address::try_from(value.address).map_err(
                |_e| Error::InvalidField("addressUnlockCondition"),
            )?))
        }
    }
}
