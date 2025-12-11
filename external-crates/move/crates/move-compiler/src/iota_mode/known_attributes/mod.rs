// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! IOTA Known Attributes

use std::{collections::BTreeSet, fmt};

use authenticator::AuthenticatorAttribute;

use crate::shared::known_attributes::{AttributePosition, KnownAttribute as MoveKnownAttribute};

pub mod authenticator;

/// The list of attribute types recognized by the compiler for the IOTA
/// Flavor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum KnownAttribute {
    Authenticator(AuthenticatorAttribute),
}

impl KnownAttribute {
    pub fn resolve(attribute_str: impl AsRef<str>) -> Option<MoveKnownAttribute> {
        Some(match attribute_str.as_ref() {
            AuthenticatorAttribute::AUTHENTICATOR => AuthenticatorAttribute.into(),
            _ => return None,
        })
    }

    pub const fn name(&self) -> &str {
        match self {
            Self::Authenticator(a) => a.name(),
        }
    }

    pub fn expected_positions(&self) -> &'static BTreeSet<AttributePosition> {
        match self {
            Self::Authenticator(a) => a.expected_positions(),
        }
    }
}

//**************************************************************************************************
// Display
//**************************************************************************************************

impl fmt::Display for KnownAttribute {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Authenticator(a) => a.fmt(f),
        }
    }
}
