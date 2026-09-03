// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! IOTA View Attribute

use std::{collections::BTreeSet, fmt};

use once_cell::sync::Lazy;

use crate::{
    expansion::ast::Attributes,
    shared::{
        ast_debug::{AstDebug, AstWriter},
        known_attributes::{AttributeKind_, AttributePosition},
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

    pub fn attribute_kind(&self) -> AttributeKind_ {
        AttributeKind_::View
    }
}

//**************************************************************************************************
// Attributes implementation
//**************************************************************************************************

impl Attributes {
    /// Returns true if the function has the `#[view]` attribute.
    pub fn is_view(&self) -> bool {
        self.contains_key_(&AttributeKind_::View)
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

//**************************************************************************************************
// AstDebug
//**************************************************************************************************

impl AstDebug for ViewAttribute {
    fn ast_debug(&self, w: &mut AstWriter) {
        w.write(self.name());
    }
}
