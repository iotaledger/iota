// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Counts the struct, enum and vector values a serialized Move value holds,
//! from its bytes and layout, without building the value.

use std::fmt;

use iota_types::error::ExecutionError;
use move_core_types::{
    annotated_value as A,
    annotated_visitor::{self, StructDriver, Traversal, VariantDriver, VecDriver},
};

/// A limit on the struct, enum and vector values in Move-call arguments, and
/// how much of it has been used.
#[derive(Debug)]
pub struct ArgumentNodeBudget {
    limit: u64,
    used: u64,
}

impl ArgumentNodeBudget {
    /// The budget for `limit`, or `None` when the protocol version sets no
    /// limit or the execution is unmetered.
    pub fn new(limit: Option<u64>, unmetered: bool) -> Option<Self> {
        if unmetered {
            return None;
        }
        limit.map(|limit| Self { limit, used: 0 })
    }

    pub fn limit(&self) -> u64 {
        self.limit
    }

    pub fn used(&self) -> u64 {
        self.used
    }

    /// What is left of the limit. `spend` keeps `used` within it, so the
    /// subtraction never saturates.
    pub fn remaining(&self) -> u64 {
        self.limit.saturating_sub(self.used)
    }

    /// Spends `nodes`. They were counted against `remaining()`, so not
    /// fitting is an invariant violation, not a limit hit.
    pub fn spend(&mut self, nodes: u64) -> Result<(), ExecutionError> {
        self.used = self
            .used
            .checked_add(nodes)
            .filter(|&used| used <= self.limit)
            .ok_or_else(|| {
                ExecutionError::invariant_violation(format!(
                    "{nodes} argument values do not fit the remaining budget of {}",
                    self.remaining()
                ))
            })?;
        Ok(())
    }
}

/// Why [`count_nodes`] stopped.
#[derive(Debug)]
pub enum NodeCountError {
    /// More than `max_nodes` values were found; the rest of the bytes were
    /// not read.
    Exceeded,
    /// The bytes do not encode a value of the layout.
    Malformed,
}

/// Counts the struct, enum and vector values in `bytes`, laid out as `layout`,
/// stopping as soon as the count passes `max_nodes`.
pub fn count_nodes(
    bytes: &[u8],
    layout: &A::MoveTypeLayout,
    max_nodes: u64,
) -> Result<u64, NodeCountError> {
    if let Some(nodes) = nodes_from_layout(layout) {
        return if nodes <= max_nodes {
            Ok(nodes)
        } else {
            Err(NodeCountError::Exceeded)
        };
    }
    let mut counter = NodeCounter {
        counted: 0,
        max_nodes,
        exceeded: false,
    };
    match A::MoveValue::visit_deserialize(bytes, layout, &mut counter) {
        Ok(()) => Ok(counter.counted),
        Err(_) if counter.exceeded => Err(NodeCountError::Exceeded),
        Err(_) => Err(NodeCountError::Malformed),
    }
}

fn nodes_from_layout(layout: &A::MoveTypeLayout) -> Option<u64> {
    match layout {
        layout if is_primitive(layout) => Some(0),
        A::MoveTypeLayout::Vector(inner) if is_primitive(inner) => Some(1),
        _ => None,
    }
}

fn is_primitive(layout: &A::MoveTypeLayout) -> bool {
    use A::MoveTypeLayout as L;
    matches!(
        layout,
        L::Bool | L::U8 | L::U16 | L::U32 | L::U64 | L::U128 | L::U256 | L::Address
    )
}

struct NodeCounter {
    counted: u64,
    max_nodes: u64,
    exceeded: bool,
}

impl NodeCounter {
    fn count_one(&mut self) -> Result<(), VisitError> {
        if self.counted >= self.max_nodes {
            self.exceeded = true;
            return Err(VisitError::Exceeded);
        }
        self.counted += 1;
        Ok(())
    }
}

#[derive(Debug)]
enum VisitError {
    Visitor(annotated_visitor::Error),
    Exceeded,
}

impl fmt::Display for VisitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Visitor(e) => e.fmt(f),
            Self::Exceeded => f.write_str("more values than allowed"),
        }
    }
}

impl std::error::Error for VisitError {}

impl From<annotated_visitor::Error> for VisitError {
    fn from(e: annotated_visitor::Error) -> Self {
        Self::Visitor(e)
    }
}

impl<'b, 'l> Traversal<'b, 'l> for NodeCounter {
    type Error = VisitError;

    fn traverse_vector(&mut self, driver: &mut VecDriver<'_, 'b, 'l>) -> Result<(), VisitError> {
        self.count_one()?;
        while driver.next_element(self)?.is_some() {}
        Ok(())
    }

    fn traverse_struct(&mut self, driver: &mut StructDriver<'_, 'b, 'l>) -> Result<(), VisitError> {
        self.count_one()?;
        while driver.next_field(self)?.is_some() {}
        Ok(())
    }

    fn traverse_variant(
        &mut self,
        driver: &mut VariantDriver<'_, 'b, 'l>,
    ) -> Result<(), VisitError> {
        self.count_one()?;
        while driver.next_field(self)?.is_some() {}
        Ok(())
    }
}
