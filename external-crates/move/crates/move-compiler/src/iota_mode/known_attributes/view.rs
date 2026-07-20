// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! IOTA View Attribute

use move_ir_types::location::Loc;
use once_cell::sync::Lazy;
use std::{collections::BTreeSet, fmt};

use crate::{
    expansion::ast::{Attribute_, Attributes},
    shared::known_attributes::{
        AttributePosition, FlavoredAttribute, KnownAttribute as MoveKnownAttribute,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ViewAttribute;

impl ViewAttribute {
    pub const VIEW: &'static str = "view";

    pub const fn name(&self) -> &'static str {
        Self::VIEW
    }

    pub fn expected_positions(&self) -> &'static BTreeSet<AttributePosition> {
        static VIEW_POSITIONS: Lazy<BTreeSet<AttributePosition>> =
            Lazy::new(|| BTreeSet::from([AttributePosition::Function]));
        &VIEW_POSITIONS
    }
}

//**************************************************************************************************
// Attribute_ implementation
//**************************************************************************************************

impl Attribute_ {
    /// Parses the view attribute.
    /// Only accepts #[view].
    pub fn parse_view_attribute(&self, loc: &Loc) -> Result<u8, (Loc, String)> {
        match self {
            Attribute_::Name(_) => Ok(1), // default version
            Attribute_::Assigned(_, _) | Attribute_::Parameterized(_, _) => Err((
                *loc,
                "Only plain #[view] attribute is supported.".to_string(),
            )),
        }
    }
}

//**************************************************************************************************
// Attributes implementation
//**************************************************************************************************

impl Attributes {
    /// Returns true if the function has the `#[view]` attribute.
    pub fn is_view(&self) -> bool {
        self.contains_key_(&MoveKnownAttribute::from(ViewAttribute))
    }
}

impl From<ViewAttribute> for MoveKnownAttribute {
    fn from(a: ViewAttribute) -> Self {
        Self::Flavored(FlavoredAttribute {
            name: a.name(),
            expected_positions: a.expected_positions(),
        })
    }
}

//**************************************************************************************************
// Display
//**************************************************************************************************

impl fmt::Display for ViewAttribute {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}
