// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! IOTA View Function Attribute
//!
//! The `#[view]` attribute marks a function as a view function, meaning it:
//! - Returns a value (non-void return type)
//! - Does not mutate on-chain state
//! - Can be called off-chain without a transaction

use std::{collections::BTreeSet, fmt};

use once_cell::sync::Lazy;

use crate::{
    expansion::ast::Attributes,
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
// Attributes implementation
//**************************************************************************************************

impl Attributes {
    /// Returns true if the function has the `#[view]` attribute.
    pub fn is_view(&self) -> bool {
        self.contains_key_(&MoveKnownAttribute::from(ViewAttribute))
    }
}

//**************************************************************************************************
// From
//**************************************************************************************************

impl From<ViewAttribute> for MoveKnownAttribute {
    fn from(v: ViewAttribute) -> Self {
        Self::Flavored(FlavoredAttribute {
            name: v.name(),
            expected_positions: v.expected_positions(),
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