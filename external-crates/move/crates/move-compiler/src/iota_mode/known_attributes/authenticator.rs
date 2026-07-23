// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! IOTA Authenticator Attribute

use std::{collections::BTreeSet, fmt};

use once_cell::sync::Lazy;

use crate::{
    expansion::ast::Attributes,
    iota_mode::known_attributes::KnownAttribute as IotaKnownAttribute,
    shared::{
        ast_debug::{AstDebug, AstWriter},
        known_attributes::{
            AttributeKind_, AttributePosition, KnownAttribute as MoveKnownAttribute,
        },
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct AuthenticatorAttribute {
    pub version: u8,
}

impl AuthenticatorAttribute {
    pub const AUTHENTICATOR: &'static str = "authenticator";
    pub const VERSION: &'static str = "version";
    pub const DEFAULT_VERSION: u8 = 1;

    pub const fn name(&self) -> &'static str {
        Self::AUTHENTICATOR
    }

    pub fn expected_positions(&self) -> &'static BTreeSet<AttributePosition> {
        static AUTHENTICATOR_POSITIONS: Lazy<BTreeSet<AttributePosition>> =
            Lazy::new(|| BTreeSet::from([AttributePosition::Function]));
        &AUTHENTICATOR_POSITIONS
    }

    pub fn attribute_kind(&self) -> AttributeKind_ {
        AttributeKind_::Authenticator
    }
}

//**************************************************************************************************
// Attributes implementation
//**************************************************************************************************

impl Attributes {
    /// Returns the version of the `#[authenticator]` attribute if present.
    pub fn get_authenticator(&self) -> Option<u8> {
        match self.get_(&AttributeKind_::Authenticator) {
            Some(
                sp!(
                    _,
                    MoveKnownAttribute::Flavored(IotaKnownAttribute::Authenticator(a))
                ),
            ) => Some(a.version),
            _ => None,
        }
    }
}

//**************************************************************************************************
// Display
//**************************************************************************************************

impl fmt::Display for AuthenticatorAttribute {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

//**************************************************************************************************
// AstDebug
//**************************************************************************************************

impl AstDebug for AuthenticatorAttribute {
    fn ast_debug(&self, w: &mut AstWriter) {
        w.write(self.name());
    }
}
