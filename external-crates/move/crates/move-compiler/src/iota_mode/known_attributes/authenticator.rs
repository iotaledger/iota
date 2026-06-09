// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! IOTA Authenticator Attribute

use std::{collections::BTreeSet, fmt};

use move_core_types::u256::U256;
use move_ir_types::location::Loc;
use move_symbol_pool::Symbol;
use once_cell::sync::Lazy;

use crate::{
    expansion::ast::{Attribute_, AttributeValue, Attributes},
    shared::known_attributes::{
        AttributePosition, FlavoredAttribute, KnownAttribute as MoveKnownAttribute,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct AuthenticatorAttribute;

impl AuthenticatorAttribute {
    pub const AUTHENTICATOR: &'static str = "authenticator";
    pub const VERSION: &'static str = "version";

    pub const fn name(&self) -> &'static str {
        Self::AUTHENTICATOR
    }

    pub fn expected_positions(&self) -> &'static BTreeSet<AttributePosition> {
        static AUTHENTICATOR_POSITIONS: Lazy<BTreeSet<AttributePosition>> =
            Lazy::new(|| BTreeSet::from([AttributePosition::Function]));
        &AUTHENTICATOR_POSITIONS
    }
}

//**************************************************************************************************
// Attribute_ implementation
//**************************************************************************************************

impl Attribute_ {
    /// Parses the authenticator attribute and returns the version number.
    /// Only accepts #[authenticator], #[authenticator = <u8>], or
    /// #[authenticator(version = <u8>)].
    pub fn parse_authenticator_version(&self, loc: &Loc) -> Result<u8, (Loc, String)> {
        use crate::expansion::ast::{AttributeName_ as AN, AttributeValue_ as AV, Value_ as V};
        fn authenticator_version(attribute_value: &AttributeValue) -> Result<u8, (Loc, String)> {
            match attribute_value {
                sp!(_, AV::Value(sp!(_, V::U8(value)))) => Ok(*value),
                sp!(_, AV::Value(sp!(_, V::InferredNum(value))))
                    if *value <= U256::from(u8::MAX) =>
                {
                    Ok(value.down_cast_lossy())
                }
                _ => Err((
                    attribute_value.loc,
                    "Only unannotated or u8 literal `version` values are supported.".to_string(),
                )),
            }
        }

        match self {
            Attribute_::Name(_) => Ok(1), // default version
            Attribute_::Assigned(_, attribute_value) => authenticator_version(&attribute_value),
            Attribute_::Parameterized(_, inner_attributes) => {
                let version_attr = inner_attributes
                    .get_(&AN::Unknown(Symbol::from(AuthenticatorAttribute::VERSION)));
                let Some(sp!(_, version_value)) = version_attr else {
                    return Err((
                        *loc,
                        "Missing `version` for authenticator attribute. Expected format: #[authenticator(version = ...)]".to_string(),
                    ));
                };
                if let Attribute_::Assigned(_, attribute_value) = version_value {
                    authenticator_version(&attribute_value)
                } else {
                    Ok(1) // default version
                }
            }
        }
    }
}

//**************************************************************************************************
// Attributes implementation
//**************************************************************************************************

impl Attributes {
    pub fn get_authenticator(&self) -> Option<u8> {
        self.get_(&MoveKnownAttribute::from(AuthenticatorAttribute))
            .and_then(|sp!(authenticator_loc, authenticator_value)| {
                authenticator_value
                    .parse_authenticator_version(authenticator_loc)
                    .ok()
            })
    }
}

//**************************************************************************************************
// From
//**************************************************************************************************

impl From<AuthenticatorAttribute> for MoveKnownAttribute {
    fn from(a: AuthenticatorAttribute) -> Self {
        Self::Flavored(FlavoredAttribute {
            name: a.name(),
            expected_positions: a.expected_positions(),
        })
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
