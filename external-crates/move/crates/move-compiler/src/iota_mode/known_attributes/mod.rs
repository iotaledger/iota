// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! IOTA Known Attributes

use std::{collections::BTreeSet, fmt};

use authenticator::AuthenticatorAttribute;
use view::ViewAttribute;

use crate::shared::{
    ast_debug::{AstDebug, AstWriter},
    known_attributes::{AttributeKind_, AttributePosition, KnownAttribute as MoveKnownAttribute},
};

pub mod authenticator;
pub mod view;

/// The list of attribute types recognized by the compiler for the IOTA
/// Flavor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum KnownAttribute {
    Authenticator(AuthenticatorAttribute),
    View(ViewAttribute),
}

impl KnownAttribute {
    pub const fn name(&self) -> &str {
        match self {
            Self::Authenticator(a) => a.name(),
            Self::View(a) => a.name(),
        }
    }

    pub fn expected_positions(&self) -> &'static BTreeSet<AttributePosition> {
        match self {
            Self::Authenticator(a) => a.expected_positions(),
            Self::View(a) => a.expected_positions(),
        }
    }

    pub fn attribute_kind(&self) -> AttributeKind_ {
        match self {
            Self::Authenticator(a) => a.attribute_kind(),
            Self::View(a) => a.attribute_kind(),
        }
    }
}

//**************************************************************************************************
// From
//**************************************************************************************************

impl From<AuthenticatorAttribute> for KnownAttribute {
    fn from(a: AuthenticatorAttribute) -> Self {
        Self::Authenticator(a)
    }
}

impl From<ViewAttribute> for KnownAttribute {
    fn from(a: ViewAttribute) -> Self {
        Self::View(a)
    }
}

impl From<KnownAttribute> for MoveKnownAttribute {
    fn from(a: KnownAttribute) -> Self {
        MoveKnownAttribute::Flavored(a)
    }
}

//**************************************************************************************************
// Display
//**************************************************************************************************

impl fmt::Display for KnownAttribute {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Authenticator(a) => a.fmt(f),
            Self::View(a) => a.fmt(f),
        }
    }
}

//**************************************************************************************************
// AstDebug
//**************************************************************************************************

impl AstDebug for KnownAttribute {
    fn ast_debug(&self, w: &mut AstWriter) {
        match self {
            Self::Authenticator(a) => a.ast_debug(w),
            Self::View(a) => a.ast_debug(w),
        }
    }
}
