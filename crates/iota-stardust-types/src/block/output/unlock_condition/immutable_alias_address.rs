// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use derive_more::From;

use crate::block::{
    Error,
    address::{Address, AliasAddress},
};

/// Defines the permanent [`AliasAddress`] that owns this output.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, From, packable::Packable)]
pub struct ImmutableAliasAddressUnlockCondition(
    #[packable(verify_with = verify_alias_address)] Address,
);

impl ImmutableAliasAddressUnlockCondition {
    /// The [`UnlockCondition`](crate::block::output::UnlockCondition)
    /// kind of an [`ImmutableAliasAddressUnlockCondition`].
    pub const KIND: u8 = 6;

    /// Creates a new [`ImmutableAliasAddressUnlockCondition`].
    #[inline(always)]
    pub fn new(address: impl Into<AliasAddress>) -> Self {
        Self(Address::Alias(address.into()))
    }

    /// Returns the address of an [`ImmutableAliasAddressUnlockCondition`].
    #[inline(always)]
    pub fn address(&self) -> &Address {
        // An ImmutableAliasAddressUnlockCondition must have an AliasAddress.
        // It has already been validated at construction that the address is an
        // `AliasAddress`.
        debug_assert!(&self.0.is_alias());
        &self.0
    }

    /// Returns the alias address of an
    /// [`ImmutableAliasAddressUnlockCondition`].
    pub fn alias_address(&self) -> &AliasAddress {
        // It has already been validated at construction that the address is an
        // `AliasAddress`.
        self.0.as_alias()
    }
}

fn verify_alias_address<const VERIFY: bool>(address: &Address, _: &()) -> Result<(), Error> {
    if VERIFY && !address.is_alias() {
        Err(Error::InvalidAddressKind(address.kind()))
    } else {
        Ok(())
    }
}
