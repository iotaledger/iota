// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod simple;

pub use self::simple::SimpleTokenScheme;
use crate::block::Error;

///
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, derive_more::From, packable::Packable)]
#[packable(unpack_error = Error)]
#[packable(tag_type = u8, with_error = Error::InvalidTokenSchemeKind)]
pub enum TokenScheme {
    ///
    #[packable(tag = SimpleTokenScheme::KIND)]
    Simple(SimpleTokenScheme),
}

impl core::fmt::Debug for TokenScheme {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Simple(scheme) => scheme.fmt(f),
        }
    }
}

impl TokenScheme {
    /// Returns the token scheme kind of a [`TokenScheme`].
    pub fn kind(&self) -> u8 {
        match self {
            Self::Simple(_) => SimpleTokenScheme::KIND,
        }
    }

    /// Checks whether the token scheme is a [`SimpleTokenScheme`].
    pub fn is_simple(&self) -> bool {
        matches!(self, Self::Simple(_))
    }

    /// Gets the token scheme as an actual [`SimpleTokenScheme`].
    /// PANIC: do not call on a non-simple token scheme.
    pub fn as_simple(&self) -> &SimpleTokenScheme {
        let Self::Simple(scheme) = self;
        scheme
    }
}
