// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! IOTA Known Attributes

use std::{collections::BTreeSet, fmt};

use crate::shared::known_attributes::{AttributePosition, KnownAttribute as MoveKnownAttribute};

/// The list of attribute types recognized by the compiler for the IOTA
/// Flavor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum KnownAttribute {}

impl KnownAttribute {
    pub fn resolve(_attribute_str: impl AsRef<str>) -> Option<MoveKnownAttribute> {
        // Some(match attribute_str.as_ref() {
        //     Attribute::ATTRIBUTE => Attribute.into(),
        //     _ => return None,
        // })
        return None;
    }

    pub const fn name(&self) -> &str {
        // match self {
        //    Self::Attribute(a) => a.name(),
        //}
        unimplemented!()
    }

    pub fn expected_positions(&self) -> &'static BTreeSet<AttributePosition> {
        // match self {
        //    Self::Attribute(a) => a.expected_positions(),
        //}
        unimplemented!()
    }
}

//**************************************************************************************************
// Display
//**************************************************************************************************

impl fmt::Display for KnownAttribute {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // match self {
        //    Self::Authenticator(a) => a.fmt(f),
        //}
        unimplemented!()
    }
}
