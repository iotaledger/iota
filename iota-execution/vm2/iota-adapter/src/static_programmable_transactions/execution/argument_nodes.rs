// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Counts the struct, enum and vector values in Move-call arguments, against the limits of the
//! protocol config, as the old executor counted them in the serialized arguments.

use iota_types::error::ExecutionError;
use move_binary_format::errors::{PartialVMError, PartialVMResult};
use move_core_types::{account_address::AccountAddress, u256::U256, vm_status::StatusCode};
use move_vm_runtime::shared::views::{ValueView, ValueVisitor};

/// A limit on the struct, enum and vector values in Move-call arguments, and how much of it has
/// been used.
#[derive(Debug)]
pub struct ArgumentNodeBudget {
    limit: u64,
    used: u64,
}

impl ArgumentNodeBudget {
    /// The budget for `limit`, or `None` when the protocol version sets no limit or the
    /// execution is unmetered.
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

    /// What is left of the limit.
    pub fn remaining(&self) -> u64 {
        self.limit.saturating_sub(self.used)
    }

    /// Spends `nodes`. They were counted against `remaining()`, so not fitting is an invariant
    /// violation, not a limit hit.
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

/// Counts the struct, enum and vector values in `value`, following references, and returns
/// `None` as soon as the count passes `max_nodes`.
pub fn count_nodes(value: &impl ValueView, max_nodes: u64) -> PartialVMResult<Option<u64>> {
    let mut counter = NodeCounter {
        counted: 0,
        max_nodes,
        exceeded: false,
    };
    match value.visit(&mut counter) {
        Ok(()) => Ok(Some(counter.counted)),
        Err(_) if counter.exceeded => Ok(None),
        Err(e) => Err(e),
    }
}

struct NodeCounter {
    counted: u64,
    max_nodes: u64,
    exceeded: bool,
}

impl NodeCounter {
    fn count_one(&mut self) -> PartialVMResult<()> {
        if self.counted >= self.max_nodes {
            self.exceeded = true;
            return Err(PartialVMError::new(StatusCode::ABORTED)
                .with_message("more argument values than allowed".to_string()));
        }
        self.counted = self.counted.saturating_add(1);
        Ok(())
    }
}

impl ValueVisitor for NodeCounter {
    fn visit_u8(&mut self, _depth: usize, _val: u8) -> PartialVMResult<()> {
        Ok(())
    }

    fn visit_u16(&mut self, _depth: usize, _val: u16) -> PartialVMResult<()> {
        Ok(())
    }

    fn visit_u32(&mut self, _depth: usize, _val: u32) -> PartialVMResult<()> {
        Ok(())
    }

    fn visit_u64(&mut self, _depth: usize, _val: u64) -> PartialVMResult<()> {
        Ok(())
    }

    fn visit_u128(&mut self, _depth: usize, _val: u128) -> PartialVMResult<()> {
        Ok(())
    }

    fn visit_u256(&mut self, _depth: usize, _val: U256) -> PartialVMResult<()> {
        Ok(())
    }

    fn visit_bool(&mut self, _depth: usize, _val: bool) -> PartialVMResult<()> {
        Ok(())
    }

    fn visit_address(&mut self, _depth: usize, _val: AccountAddress) -> PartialVMResult<()> {
        Ok(())
    }

    fn visit_struct(&mut self, _depth: usize, _len: usize) -> PartialVMResult<bool> {
        self.count_one()?;
        Ok(true)
    }

    fn visit_variant(&mut self, _depth: usize, _len: usize) -> PartialVMResult<bool> {
        self.count_one()?;
        Ok(true)
    }

    fn visit_vec(&mut self, _depth: usize, _len: usize) -> PartialVMResult<bool> {
        self.count_one()?;
        Ok(true)
    }

    fn visit_ref(&mut self, _depth: usize) -> PartialVMResult<bool> {
        // The old executor passed a borrowed argument as the bytes of the value it points to.
        Ok(true)
    }

    // A vector of primitive values is one value, whatever its length.
    fn visit_vec_u8(&mut self, _depth: usize, _vals: &[u8]) -> PartialVMResult<()> {
        self.count_one()
    }

    fn visit_vec_u16(&mut self, _depth: usize, _vals: &[u16]) -> PartialVMResult<()> {
        self.count_one()
    }

    fn visit_vec_u32(&mut self, _depth: usize, _vals: &[u32]) -> PartialVMResult<()> {
        self.count_one()
    }

    fn visit_vec_u64(&mut self, _depth: usize, _vals: &[u64]) -> PartialVMResult<()> {
        self.count_one()
    }

    fn visit_vec_u128(&mut self, _depth: usize, _vals: &[u128]) -> PartialVMResult<()> {
        self.count_one()
    }

    fn visit_vec_u256(&mut self, _depth: usize, _vals: &[U256]) -> PartialVMResult<()> {
        self.count_one()
    }

    fn visit_vec_bool(&mut self, _depth: usize, _vals: &[bool]) -> PartialVMResult<()> {
        self.count_one()
    }

    fn visit_vec_address(
        &mut self,
        _depth: usize,
        _vals: &[AccountAddress],
    ) -> PartialVMResult<()> {
        self.count_one()
    }
}
