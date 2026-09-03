// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeMap;

use move_binary_format::errors::{PartialVMError, PartialVMResult};
use move_core_types::{
    gas_algebra::{AbstractMemorySize, InternalGas},
    vm_status::StatusCode,
};
use move_vm_profiler::GasProfiler;
use once_cell::sync::Lazy;

use crate::gas_model::{
    resource_profile::ResourceProfile,
    units_types::{CostTable, Gas, GasCost},
};

/// VM flat fee
pub const VM_FLAT_FEE: Gas = Gas::new(8_000);

/// The size in bytes for a non-string or address constant on the stack
pub const CONST_SIZE: AbstractMemorySize = AbstractMemorySize::new(16);

/// The size in bytes for a reference on the stack
pub const REFERENCE_SIZE: AbstractMemorySize = AbstractMemorySize::new(8);

/// The size of a struct in bytes
pub const STRUCT_SIZE: AbstractMemorySize = AbstractMemorySize::new(2);

/// The size of a vector (without its containing data) in bytes
pub const VEC_SIZE: AbstractMemorySize = AbstractMemorySize::new(8);

/// For exists checks on data that doesn't exists this is the multiplier that is
/// used.
pub const MIN_EXISTS_DATA_SIZE: AbstractMemorySize = AbstractMemorySize::new(100);

pub static ZERO_COST_SCHEDULE: Lazy<CostTable> = Lazy::new(zero_cost_schedule);

pub static INITIAL_COST_SCHEDULE: Lazy<CostTable> = Lazy::new(initial_cost_schedule_v1);

/// The Move VM implementation of state for gas metering.
///
/// Initialize with a `CostTable` and the gas provided to the transaction.
/// Provide all the proper guarantees about gas metering in the Move VM.
///
/// Every client must use an instance of this type to interact with the Move VM.
#[derive(Debug)]
pub struct GasStatus {
    pub gas_model_version: u64,
    cost_table: CostTable,
    pub gas_left: InternalGas,
    gas_price: u64,
    initial_budget: InternalGas,
    pub charge: bool,

    // The current height of the operand stack, and the maximal height that it has reached.
    stack_height_high_water_mark: u64,
    stack_height_current: u64,
    stack_height_next_tier_start: Option<u64>,
    stack_height_current_tier_mult: u64,

    // The current (abstract) size  of the operand stack and the maximal size that it has reached.
    stack_size_high_water_mark: u64,
    stack_size_current: u64,
    stack_size_next_tier_start: Option<u64>,
    stack_size_current_tier_mult: u64,

    // The total number of bytecode instructions that have been executed in the transaction.
    instructions_executed: u64,
    instructions_next_tier_start: Option<u64>,
    instructions_current_tier_mult: u64,

    pub profiler: Option<GasProfiler>,
    pub num_native_calls: u64,

    // Counters feeding `ResourceProfile`. Reading or updating them does not
    // change the gas charged, and they are derived from deterministic
    // quantities (sizes and counts), so they are identical on every validator.
    locals_size_current: u64,
    locals_size_high_water_mark: u64,
    // Abstract bytes added to each live frame's locals through the store/call
    // hooks, innermost frame last. At frame drop, any excess of the dropped
    // values' size over the frame's tracked additions is growth that happened
    // in place through references (e.g. `vector::push_back`), invisible to the
    // hooks; it is recorded late so the locals high-water mark includes it and
    // the running size never deflates below the other frames' contributions.
    frame_locals_added: Vec<u64>,
    // True peak of the operand stack's abstract size: unlike the charged
    // `stack_size_current` above (whose decreases are intentionally not
    // applied, preserving today's tier escalation), this pair applies both
    // increases and decreases, giving the peak resident size for the
    // working-memory profile.
    profile_stack_size_current: u64,
    profile_stack_size_peak: u64,
    // Internal gas actually deducted (tiering-correct) for native calls and
    // for the two per-byte categories (storage reads vs. package
    // publish/upgrade bytes, which use different rates). The interpreter
    // share is the remainder: `total_deducted - native - per-byte`. Tracking
    // the non-interpreter categories by delta keeps the per-instruction
    // charging path untouched.
    native_gas_deducted: u64,
    storage_read_gas_deducted: u64,
    publish_gas_deducted: u64,
    // The three untiered components of interpreter gas, accumulated so each can
    // be calibrated separately: instruction dispatch count, operand-stack bytes
    // moved, and pushes. Interpreter-only (native's share is subtracted via
    // `discount_native_flows`).
    interp_instr_flow: u64,
    interp_stack_size_flow: u64,
    interp_stack_height_flow: u64,
    // Module id + function name of the native currently executing; attributes
    // the next `record_native_gas_deducted` into the per-function maps.
    pending_native_function: String,
    native_gas_by_function: BTreeMap<String, u64>,
    native_calls_by_function: BTreeMap<String, u64>,
    input_object_count: u64,
    input_object_bytes: u64,
    child_object_reads: u64,
    child_object_read_bytes: u64,
    object_runtime_cached_bytes: u64,
    packages_loaded: u64,
    package_bytes_loaded: u64,
    event_count: u64,
    event_bytes: u64,
    hash_input_bytes: u64,
}

impl GasStatus {
    /// Initialize the gas state with metering enabled.
    ///
    /// Charge for every operation and fail when there is no more gas to pay for
    /// operations. This is the instantiation that must be used when
    /// executing a user script.
    pub fn new(cost_table: CostTable, budget: u64, gas_price: u64, gas_model_version: u64) -> Self {
        assert!(gas_price > 0, "gas price cannot be 0");
        let budget_in_unit = budget / gas_price;
        let gas_left = Self::to_internal_units(budget_in_unit);

        let (stack_height_current_tier_mult, stack_height_next_tier_start) =
            cost_table.stack_height_tier(0);
        let (stack_size_current_tier_mult, stack_size_next_tier_start) =
            cost_table.stack_size_tier(0);
        let (instructions_current_tier_mult, instructions_next_tier_start) =
            cost_table.instruction_tier(0);
        Self {
            gas_model_version,
            gas_left,
            gas_price,
            initial_budget: gas_left,
            cost_table,
            charge: true,
            stack_height_high_water_mark: 0,
            stack_height_current: 0,
            stack_size_high_water_mark: 0,
            stack_size_current: 0,
            instructions_executed: 0,
            stack_height_current_tier_mult,
            stack_size_current_tier_mult,
            instructions_current_tier_mult,
            stack_height_next_tier_start,
            stack_size_next_tier_start,
            instructions_next_tier_start,
            profiler: None,
            num_native_calls: 0,
            locals_size_current: 0,
            locals_size_high_water_mark: 0,
            frame_locals_added: Vec::new(),
            profile_stack_size_current: 0,
            profile_stack_size_peak: 0,
            native_gas_deducted: 0,
            storage_read_gas_deducted: 0,
            publish_gas_deducted: 0,
            interp_instr_flow: 0,
            interp_stack_size_flow: 0,
            interp_stack_height_flow: 0,
            pending_native_function: String::new(),
            native_gas_by_function: BTreeMap::new(),
            native_calls_by_function: BTreeMap::new(),
            input_object_count: 0,
            input_object_bytes: 0,
            child_object_reads: 0,
            child_object_read_bytes: 0,
            object_runtime_cached_bytes: 0,
            packages_loaded: 0,
            package_bytes_loaded: 0,
            event_count: 0,
            event_bytes: 0,
            hash_input_bytes: 0,
        }
    }

    /// Initialize the gas state with metering disabled.
    ///
    /// It should be used by clients in very specific cases and when executing
    /// system code that does not have to charge the user.
    pub fn new_unmetered() -> Self {
        Self {
            gas_model_version: 1,
            gas_left: InternalGas::new(0),
            gas_price: 1,
            initial_budget: InternalGas::new(0),
            cost_table: ZERO_COST_SCHEDULE.clone(),
            charge: false,
            stack_height_high_water_mark: 0,
            stack_height_current: 0,
            stack_size_high_water_mark: 0,
            stack_size_current: 0,
            instructions_executed: 0,
            stack_height_current_tier_mult: 0,
            stack_size_current_tier_mult: 0,
            instructions_current_tier_mult: 0,
            stack_height_next_tier_start: None,
            stack_size_next_tier_start: None,
            instructions_next_tier_start: None,
            profiler: None,
            num_native_calls: 0,
            locals_size_current: 0,
            locals_size_high_water_mark: 0,
            frame_locals_added: Vec::new(),
            profile_stack_size_current: 0,
            profile_stack_size_peak: 0,
            native_gas_deducted: 0,
            storage_read_gas_deducted: 0,
            publish_gas_deducted: 0,
            interp_instr_flow: 0,
            interp_stack_size_flow: 0,
            interp_stack_height_flow: 0,
            pending_native_function: String::new(),
            native_gas_by_function: BTreeMap::new(),
            native_calls_by_function: BTreeMap::new(),
            input_object_count: 0,
            input_object_bytes: 0,
            child_object_reads: 0,
            child_object_read_bytes: 0,
            object_runtime_cached_bytes: 0,
            packages_loaded: 0,
            package_bytes_loaded: 0,
            event_count: 0,
            event_bytes: 0,
            hash_input_bytes: 0,
        }
    }

    const INTERNAL_UNIT_MULTIPLIER: u64 = 1000;

    fn to_internal_units(val: u64) -> InternalGas {
        InternalGas::new(val * Self::INTERNAL_UNIT_MULTIPLIER)
    }

    #[expect(dead_code)]
    fn to_nanos(&self, val: InternalGas) -> u64 {
        let gas: Gas = InternalGas::to_unit_round_down(val);
        u64::from(gas) * self.gas_price
    }

    pub fn push_stack(&mut self, pushes: u64) -> PartialVMResult<()> {
        match self.stack_height_current.checked_add(pushes) {
            // We should never hit this.
            None => return Err(PartialVMError::new(StatusCode::ARITHMETIC_OVERFLOW)),
            Some(new_height) => {
                if new_height > self.stack_height_high_water_mark {
                    self.stack_height_high_water_mark = new_height;
                }
                self.stack_height_current = new_height;
            }
        }

        if let Some(stack_height_tier_next) = self.stack_height_next_tier_start {
            if self.stack_height_current > stack_height_tier_next {
                let (next_mul, next_tier) =
                    self.cost_table.stack_height_tier(self.stack_height_current);
                self.stack_height_current_tier_mult = next_mul;
                self.stack_height_next_tier_start = next_tier;
            }
        }

        Ok(())
    }

    pub fn pop_stack(&mut self, pops: u64) {
        self.stack_height_current = self.stack_height_current.saturating_sub(pops);
    }

    pub fn increase_instruction_count(&mut self, amount: u64) -> PartialVMResult<()> {
        match self.instructions_executed.checked_add(amount) {
            None => return Err(PartialVMError::new(StatusCode::PC_OVERFLOW)),
            Some(new_pc) => {
                self.instructions_executed = new_pc;
            }
        }

        if let Some(instr_tier_next) = self.instructions_next_tier_start {
            if self.instructions_executed > instr_tier_next {
                let (instr_cost, next_tier) =
                    self.cost_table.instruction_tier(self.instructions_executed);
                self.instructions_current_tier_mult = instr_cost;
                self.instructions_next_tier_start = next_tier;
            }
        }

        Ok(())
    }

    pub fn increase_stack_size(&mut self, size_amount: u64) -> PartialVMResult<()> {
        match self.stack_size_current.checked_add(size_amount) {
            None => return Err(PartialVMError::new(StatusCode::ARITHMETIC_OVERFLOW)),
            Some(new_size) => {
                if new_size > self.stack_size_high_water_mark {
                    self.stack_size_high_water_mark = new_size;
                }
                self.stack_size_current = new_size;
            }
        }

        if let Some(stack_size_tier_next) = self.stack_size_next_tier_start {
            if self.stack_size_current > stack_size_tier_next {
                let (next_mul, next_tier) =
                    self.cost_table.stack_size_tier(self.stack_size_current);
                self.stack_size_current_tier_mult = next_mul;
                self.stack_size_next_tier_start = next_tier;
            }
        }

        Ok(())
    }

    pub fn decrease_stack_size(&mut self, size_amount: u64) {
        let new_size = self.stack_size_current.saturating_sub(size_amount);
        if new_size > self.stack_size_high_water_mark {
            self.stack_size_high_water_mark = new_size;
        }
        self.stack_size_current = new_size;
    }

    /// Given: pushes + pops + increase + decrease in size for an instruction
    /// charge for the execution of the instruction.
    pub fn charge(
        &mut self,
        num_instructions: u64,
        pushes: u64,
        pops: u64,
        incr_size: u64,
        decr_size: u64,
    ) -> PartialVMResult<()> {
        // Accumulate the three untiered components that make up interpreter gas
        // — instruction dispatch, operand-stack bytes moved, and pushes — so
        // the profile can reprice each separately. Native calls also route
        // through `charge`; the meter subtracts their share via
        // `discount_native_flows`, leaving these interpreter-only.
        self.interp_instr_flow = self.interp_instr_flow.saturating_add(num_instructions);
        self.interp_stack_size_flow = self.interp_stack_size_flow.saturating_add(incr_size);
        self.interp_stack_height_flow = self.interp_stack_height_flow.saturating_add(pushes);

        // Maintain the true operand-stack size peak for the resource profile.
        // `decr_size` is applied here and only here: the charged
        // `stack_size_current` below intentionally ignores decreases (that is
        // what today's tiers escalate on), so it must stay untouched.
        self.profile_stack_size_current = self.profile_stack_size_current.saturating_add(incr_size);
        if self.profile_stack_size_current > self.profile_stack_size_peak {
            self.profile_stack_size_peak = self.profile_stack_size_current;
        }
        self.profile_stack_size_current = self.profile_stack_size_current.saturating_sub(decr_size);

        self.push_stack(pushes)?;
        self.increase_instruction_count(num_instructions)?;
        self.increase_stack_size(incr_size)?;

        self.deduct_gas(
            GasCost::new(
                self.instructions_current_tier_mult
                    .checked_mul(num_instructions)
                    .ok_or_else(|| PartialVMError::new(StatusCode::ARITHMETIC_OVERFLOW))?,
                self.stack_size_current_tier_mult
                    .checked_mul(incr_size)
                    .ok_or_else(|| PartialVMError::new(StatusCode::ARITHMETIC_OVERFLOW))?,
                self.stack_height_current_tier_mult
                    .checked_mul(pushes)
                    .ok_or_else(|| PartialVMError::new(StatusCode::ARITHMETIC_OVERFLOW))?,
            )
            .total_internal(),
        )?;

        // self.decrease_stack_size(decr_size);
        self.pop_stack(pops);
        Ok(())
    }

    /// Return the `CostTable` behind this `GasStatus`.
    pub fn cost_table(&self) -> &CostTable {
        &self.cost_table
    }

    /// Return the gas left.
    pub fn remaining_gas(&self) -> Gas {
        self.gas_left.to_unit_round_down()
    }

    /// Charge a given amount of gas and fail if not enough gas units are left.
    pub fn deduct_gas(&mut self, amount: InternalGas) -> PartialVMResult<()> {
        if !self.charge {
            return Ok(());
        }

        match self.gas_left.checked_sub(amount) {
            Some(gas_left) => {
                self.gas_left = gas_left;
                Ok(())
            }
            None => {
                self.gas_left = InternalGas::new(0);
                Err(PartialVMError::new(StatusCode::OUT_OF_GAS))
            }
        }
    }

    pub fn record_native_call(&mut self) {
        self.num_native_calls = self.num_native_calls.saturating_add(1);
    }

    // Deduct the amount provided with no conversion, as if it was InternalGasUnit
    fn deduct_units(&mut self, amount: u64) -> PartialVMResult<()> {
        self.deduct_gas(InternalGas::new(amount))
    }

    pub fn set_metering(&mut self, enabled: bool) {
        self.charge = enabled
    }

    // The amount of gas used, it does not include the multiplication for the gas
    // price
    pub fn gas_used_pre_gas_price(&self) -> u64 {
        let gas: Gas = match self.initial_budget.checked_sub(self.gas_left) {
            Some(val) => InternalGas::to_unit_round_down(val),
            None => InternalGas::to_unit_round_down(self.initial_budget),
        };
        u64::from(gas)
    }

    // Charge the number of bytes with the cost per byte value
    // As more bytes are read throughout the computation the cost per bytes is
    // increased.
    pub fn charge_bytes(&mut self, size: usize, cost_per_byte: u64) -> PartialVMResult<()> {
        let computation_cost = size as u64 * cost_per_byte;
        let gas_before = self.gas_left;
        let result = self.deduct_units(computation_cost);
        // Attribute the internal gas actually deducted (partial on
        // out-of-gas, zero when unmetered) to the storage-read per-byte
        // category, so it is excluded from the interpreter share of the
        // resource profile.
        self.storage_read_gas_deducted = self
            .storage_read_gas_deducted
            .saturating_add(Self::gas_delta(gas_before, self.gas_left));
        result
    }

    /// Like [`charge_bytes`](Self::charge_bytes), but attributes the deducted
    /// gas to the package publish/upgrade per-byte category instead of
    /// storage reads. The two categories use different per-byte rates and
    /// price different work, so the profile keeps them apart.
    pub fn charge_publish_bytes(&mut self, size: usize, cost_per_byte: u64) -> PartialVMResult<()> {
        let computation_cost = size as u64 * cost_per_byte;
        let gas_before = self.gas_left;
        let result = self.deduct_units(computation_cost);
        self.publish_gas_deducted = self
            .publish_gas_deducted
            .saturating_add(Self::gas_delta(gas_before, self.gas_left));
        result
    }

    /// Internal gas deducted between two `gas_left` readings (`before - after`,
    /// saturating at zero).
    fn gas_delta(before: InternalGas, after: InternalGas) -> u64 {
        u64::from(
            before
                .checked_sub(after)
                .unwrap_or_else(|| InternalGas::new(0)),
        )
    }

    pub fn gas_price(&self) -> u64 {
        self.gas_price
    }

    pub fn stack_height_high_water_mark(&self) -> u64 {
        self.stack_height_high_water_mark
    }

    pub fn stack_size_high_water_mark(&self) -> u64 {
        self.stack_size_high_water_mark
    }

    pub fn instructions_executed(&self) -> u64 {
        self.instructions_executed
    }

    /// Record that `amount` abstract bytes moved into frame locals. Locals are
    /// not charged by gas; this records the bytes for the working-memory
    /// component of [`ResourceProfile`].
    fn increase_locals_size(&mut self, amount: u64) {
        self.locals_size_current = self.locals_size_current.saturating_add(amount);
        if self.locals_size_current > self.locals_size_high_water_mark {
            self.locals_size_high_water_mark = self.locals_size_current;
        }
    }

    /// Record that `amount` abstract bytes left frame locals (moved out or
    /// dropped with the frame).
    fn decrease_locals_size(&mut self, amount: u64) {
        self.locals_size_current = self.locals_size_current.saturating_sub(amount);
    }

    /// Add `amount` to the innermost live frame's tracked locals additions,
    /// opening an implicit root frame if none is live. The entry function's
    /// frame gets no `record_call_frame` (only `Call` instructions do), so its
    /// stores land in the implicit root.
    fn add_to_current_frame(&mut self, amount: u64) {
        match self.frame_locals_added.last_mut() {
            Some(top) => *top = top.saturating_add(amount),
            None => self.frame_locals_added.push(amount),
        }
    }

    /// Record a function call: `args_size` abstract bytes move from the
    /// operand stack into the callee frame's locals, and a new frame becomes
    /// live for per-frame growth tracking.
    pub fn record_call_frame(&mut self, args_size: u64) {
        self.increase_locals_size(args_size);
        self.frame_locals_added.push(args_size);
    }

    /// Record a value of `size` abstract bytes stored from the operand stack
    /// into a local. Storing over an occupied local over-counts (the
    /// displaced value is not visible here), which is conservative for a
    /// high-water mark.
    pub fn record_store_loc(&mut self, size: u64) {
        self.increase_locals_size(size);
        self.add_to_current_frame(size);
    }

    /// Record a value of `size` abstract bytes moved out of a local onto the
    /// operand stack.
    pub fn record_move_loc(&mut self, size: u64) {
        self.decrease_locals_size(size);
        if let Some(top) = self.frame_locals_added.last_mut() {
            *top = top.saturating_sub(size);
        }
    }

    /// Record a frame drop: `dropped_size` is the total abstract size of the
    /// non-reference values still in the frame's locals. Values grown in
    /// place through references (e.g. `vector::push_back` via `&mut`) never
    /// passed the store/move hooks, so any excess of `dropped_size` over the
    /// frame's tracked additions is growth observed late: it is added first
    /// (raising the high-water mark to include it) and then the whole dropped
    /// size is removed, so the running size never deflates below the still-
    /// live frames' contributions.
    pub fn record_drop_frame(&mut self, dropped_size: u64) {
        let tracked = self.frame_locals_added.pop().unwrap_or(0);
        if dropped_size > tracked {
            self.increase_locals_size(dropped_size - tracked);
        }
        self.decrease_locals_size(dropped_size);
    }

    /// Record the distinct non-system packages fetched for this transaction,
    /// for the read-I/O component of [`ResourceProfile`].
    pub fn record_package_loads(&mut self, count: u64, bytes: u64) {
        self.packages_loaded = count;
        self.package_bytes_loaded = bytes;
    }

    /// Record the identity (module id + function name) of the native about
    /// to execute; the next
    /// [`record_native_gas_deducted`](Self::record_native_gas_deducted) is
    /// attributed to it.
    pub fn set_pending_native_function(&mut self, module_id: &str, function_name: &str) {
        self.pending_native_function.clear();
        self.pending_native_function.push_str(module_id);
        self.pending_native_function.push_str("::");
        self.pending_native_function.push_str(function_name);
    }

    /// True when the pending native (set by `set_pending_native_function`)
    /// is one whose input is streamed byte-by-byte: the `0x1::hash` and
    /// `0x2::hash` families and `0x2::hmac`. System addresses, so no user
    /// package can alias these module ids.
    pub fn pending_native_streams_input(&self) -> bool {
        ["0x1::hash::", "0x2::hash::", "0x2::hmac::"]
            .iter()
            .any(|m| self.pending_native_function.starts_with(m))
    }

    /// Record the abstract size of a hashing native's arguments, for the
    /// memory-bandwidth component of [`ResourceProfile`]. Profile-only;
    /// charges nothing.
    pub fn record_hash_input_bytes(&mut self, bytes: u64) {
        self.hash_input_bytes = self.hash_input_bytes.saturating_add(bytes);
    }

    /// Attribute the internal gas deducted for a native call to the pending
    /// native function. Call with the `gas_left` reading captured *before*
    /// the native's charges; the delta is the gas actually deducted (tiering-
    /// correct, and partial on out-of-gas). This observes charging that has
    /// already happened; it does not itself charge gas.
    pub fn record_native_gas_deducted(&mut self, gas_left_before: InternalGas) {
        let deducted = Self::gas_delta(gas_left_before, self.gas_left);
        self.native_gas_deducted = self.native_gas_deducted.saturating_add(deducted);
        // Look up before inserting so the key is cloned only once per
        // distinct function, not on every native call.
        if let Some(per_function) = self
            .native_gas_by_function
            .get_mut(&self.pending_native_function)
        {
            *per_function = per_function.saturating_add(deducted);
        } else {
            self.native_gas_by_function
                .insert(self.pending_native_function.clone(), deducted);
        }
        if let Some(calls) = self
            .native_calls_by_function
            .get_mut(&self.pending_native_function)
        {
            *calls = calls.saturating_add(1);
        } else {
            self.native_calls_by_function
                .insert(self.pending_native_function.clone(), 1);
        }
    }

    /// Subtract a native call's contribution from the interpreter component
    /// flows. Native calls route through [`charge`](Self::charge), which adds
    /// to those flows; the meter calls this afterward with the same
    /// `(num_instructions, pushes, incr_size)` the native charge used, leaving
    /// the flows interpreter-only.
    pub fn discount_native_flows(&mut self, num_instructions: u64, pushes: u64, incr_size: u64) {
        self.interp_instr_flow = self.interp_instr_flow.saturating_sub(num_instructions);
        self.interp_stack_size_flow = self.interp_stack_size_flow.saturating_sub(incr_size);
        self.interp_stack_height_flow = self.interp_stack_height_flow.saturating_sub(pushes);
    }

    /// Record the input objects loaded before execution, for the read-I/O
    /// component of [`ResourceProfile`].
    pub fn record_input_objects(&mut self, count: u64, bytes: u64) {
        self.input_object_count = count;
        self.input_object_bytes = bytes;
    }

    /// Record the object runtime's child-object load counters at the end of
    /// execution, for the read-I/O and working-memory components of
    /// [`ResourceProfile`].
    pub fn record_object_runtime_usage(&mut self, reads: u64, read_bytes: u64, cached_bytes: u64) {
        self.child_object_reads = reads;
        self.child_object_read_bytes = read_bytes;
        self.object_runtime_cached_bytes = cached_bytes;
    }

    /// Record the emitted events' count and total serialized size at the end
    /// of execution, for the commit-write and cardinality components of
    /// [`ResourceProfile`].
    pub fn record_events(&mut self, count: u64, bytes: u64) {
        self.event_count = count;
        self.event_bytes = bytes;
    }

    /// Assemble the VM-side portion of the per-transaction
    /// [`ResourceProfile`]. The write-side fields (written/deleted objects)
    /// are not visible here and are filled in by the caller from storage
    /// tracking.
    pub fn resource_profile(&self) -> ResourceProfile {
        // Interpreter gas is the part of the computation total that is neither
        // a native call nor a per-byte charge. All categories are internal gas
        // units, so they sum exactly to the total deducted.
        let total_deducted = Self::gas_delta(self.initial_budget, self.gas_left);
        let interpreter_gas = total_deducted
            .saturating_sub(self.native_gas_deducted)
            .saturating_sub(self.storage_read_gas_deducted)
            .saturating_sub(self.publish_gas_deducted);
        ResourceProfile {
            instructions_executed: self.instructions_executed,
            num_native_calls: self.num_native_calls,
            interpreter_gas,
            interp_instruction_count: self.interp_instr_flow,
            interp_stack_size_flow: self.interp_stack_size_flow,
            interp_stack_height_flow: self.interp_stack_height_flow,
            native_gas: self.native_gas_deducted,
            native_gas_by_function: self.native_gas_by_function.clone(),
            native_calls_by_function: self.native_calls_by_function.clone(),
            storage_read_gas: self.storage_read_gas_deducted,
            package_publish_gas: self.publish_gas_deducted,
            computation_gas_used: self.gas_used_pre_gas_price(),
            stack_size_high_water_mark: self.profile_stack_size_peak,
            stack_height_high_water_mark: self.stack_height_high_water_mark,
            locals_size_high_water_mark: self.locals_size_high_water_mark,
            object_runtime_cached_bytes: self.object_runtime_cached_bytes,
            input_object_count: self.input_object_count,
            input_object_bytes: self.input_object_bytes,
            child_object_reads: self.child_object_reads,
            packages_loaded: self.packages_loaded,
            package_bytes_loaded: self.package_bytes_loaded,
            child_object_read_bytes: self.child_object_read_bytes,
            written_object_count: 0,
            written_bytes: 0,
            deleted_object_count: 0,
            event_count: self.event_count,
            event_bytes: self.event_bytes,
            hash_input_bytes: self.hash_input_bytes,
        }
    }
}

pub fn zero_cost_schedule() -> CostTable {
    let mut zero_tier = BTreeMap::new();
    zero_tier.insert(0, 0);
    CostTable {
        instruction_tiers: zero_tier.clone(),
        stack_size_tiers: zero_tier.clone(),
        stack_height_tiers: zero_tier,
    }
}

pub fn unit_cost_schedule() -> CostTable {
    let mut unit_tier = BTreeMap::new();
    unit_tier.insert(0, 1);
    CostTable {
        instruction_tiers: unit_tier.clone(),
        stack_size_tiers: unit_tier.clone(),
        stack_height_tiers: unit_tier,
    }
}

pub fn initial_cost_schedule_v1() -> CostTable {
    let instruction_tiers: BTreeMap<u64, u64> = vec![
        (0, 1),
        (20_000, 2),
        (50_000, 10),
        (100_000, 50),
        (200_000, 100),
        (10_000_000, 1000),
    ]
    .into_iter()
    .collect();

    let stack_height_tiers: BTreeMap<u64, u64> =
        vec![(0, 1), (1_000, 2), (10_000, 10)].into_iter().collect();

    let stack_size_tiers: BTreeMap<u64, u64> = vec![
        (0, 1),
        (100_000, 2),        // ~100K
        (500_000, 5),        // ~500K
        (1_000_000, 100),    // ~1M
        (100_000_000, 1000), // ~100M
    ]
    .into_iter()
    .collect();

    CostTable {
        instruction_tiers,
        stack_size_tiers,
        stack_height_tiers,
    }
}

// Convert from our representation of gas costs to the type that the MoveVM
// expects for unit tests. We don't want our gas depending on the MoveVM test
// utils and we don't want to fix our representation to whatever is there, so
// instead we perform this translation from our gas units and cost schedule to
// the one expected by the Move unit tests.
pub fn initial_cost_schedule_for_unit_tests() -> move_vm_test_utils::gas_schedule::CostTable {
    let table = initial_cost_schedule_v1();
    move_vm_test_utils::gas_schedule::CostTable {
        instruction_tiers: table.instruction_tiers,
        stack_height_tiers: table.stack_height_tiers,
        stack_size_tiers: table.stack_size_tiers,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locals_size_tracking_records_high_water_mark() {
        let mut status = GasStatus::new_unmetered();
        status.increase_locals_size(100);
        status.increase_locals_size(50);
        status.decrease_locals_size(120);
        status.increase_locals_size(30);
        assert_eq!(status.resource_profile().locals_size_high_water_mark, 150);

        // Draining below zero saturates instead of wrapping.
        status.decrease_locals_size(u64::MAX);
        status.increase_locals_size(10);
        assert_eq!(status.resource_profile().locals_size_high_water_mark, 150);
    }

    #[test]
    fn recorded_counters_surface_in_resource_profile() {
        let mut status = GasStatus::new_unmetered();
        status.record_input_objects(3, 400);
        status.record_object_runtime_usage(4, 1000, 900);
        status.record_events(2, 64);

        let profile = status.resource_profile();
        assert_eq!(profile.input_object_count, 3);
        assert_eq!(profile.input_object_bytes, 400);
        assert_eq!(profile.child_object_reads, 4);
        assert_eq!(profile.child_object_read_bytes, 1000);
        assert_eq!(profile.object_runtime_cached_bytes, 900);
        assert_eq!(profile.event_count, 2);
        assert_eq!(profile.event_bytes, 64);
        // Write-side fields are filled by the storage-tracking layer, not
        // here.
        assert_eq!(profile.written_object_count, 0);
        assert_eq!(profile.written_bytes, 0);
        assert_eq!(profile.deleted_object_count, 0);
    }

    #[test]
    fn gas_split_into_interpreter_native_and_byte() {
        let cost_table = initial_cost_schedule_v1();
        let mut status = GasStatus::new(cost_table, 1_000_000, 1, 1);

        // An interpreter charge (bytecode instruction + stack ops). Measure
        // the gas it deducts independently.
        let before_interp = u64::from(status.gas_left);
        status.charge(1, 1, 0, 0, 0).unwrap();
        let interp = before_interp - u64::from(status.gas_left);
        assert!(interp > 0, "interpreter charge should deduct gas");

        // A native call: the adapter's meter captures gas_left before the
        // native's charges and records the delta against the pending module.
        status.set_pending_native_function("0x2::ed25519", "ed25519_verify");
        let before_native = status.gas_left;
        status.deduct_gas(InternalGas::new(5000)).unwrap();
        status.record_native_gas_deducted(before_native);

        // A second native in a different module.
        status.set_pending_native_function("0x2::bls12381", "bls12381_min_sig_verify");
        let before_native2 = status.gas_left;
        status.deduct_gas(InternalGas::new(2000)).unwrap();
        status.record_native_gas_deducted(before_native2);

        // A per-byte charge (storage read): excluded from the interpreter
        // share.
        status.charge_bytes(10, 3).unwrap();

        let profile = status.resource_profile();
        assert_eq!(profile.native_gas, 7000);
        assert_eq!(
            profile
                .native_gas_by_function
                .get("0x2::ed25519::ed25519_verify"),
            Some(&5000)
        );
        assert_eq!(
            profile
                .native_gas_by_function
                .get("0x2::bls12381::bls12381_min_sig_verify"),
            Some(&2000)
        );
        assert_eq!(
            profile
                .native_calls_by_function
                .get("0x2::ed25519::ed25519_verify"),
            Some(&1)
        );
        // Interpreter gas = total deducted − native − byte, so it recovers
        // exactly the interpreter charge and excludes the 7000 native and the
        // 30 byte gas.
        assert_eq!(profile.interpreter_gas, interp);
    }

    #[test]
    fn interpreter_component_flows_exclude_native() {
        let cost_table = initial_cost_schedule_v1();
        let mut status = GasStatus::new(cost_table, 1_000_000, 1, 1);

        // Two interpreter charges: (2 instructions, 3 pushes, 40 size) and
        // (1 instruction, 0 pushes, 8 size).
        status.charge(2, 3, 0, 40, 0).unwrap();
        status.charge(1, 0, 0, 8, 0).unwrap();

        // Simulate an above-threshold native call, which routes through
        // `charge` with the amount as instruction count, then discounts its
        // own share so the interpreter flows stay interpreter-only.
        status.charge(500, 2, 0, 16, 0).unwrap();
        status.discount_native_flows(500, 2, 16);

        let profile = status.resource_profile();
        assert_eq!(profile.interp_instruction_count, 3); // 2 + 1, native 500 discounted
        assert_eq!(profile.interp_stack_height_flow, 3); // 3 + 0, native 2 discounted
        assert_eq!(profile.interp_stack_size_flow, 48); // 40 + 8, native 16 discounted
    }

    #[test]
    fn measurement_counters_do_not_affect_charging() {
        let cost_table = initial_cost_schedule_v1();
        let mut status = GasStatus::new(cost_table, 1_000_000, 1, 1);
        let before = status.remaining_gas();
        status.increase_locals_size(1_000_000);
        status.record_input_objects(10, 10_000);
        status.record_object_runtime_usage(10, 10_000, 10_000);
        status.record_events(10, 10_000);
        status.record_package_loads(3, 30_000);
        status.record_call_frame(500);
        status.record_store_loc(100);
        status.record_drop_frame(600);
        assert_eq!(status.remaining_gas(), before);
        assert_eq!(status.gas_used_pre_gas_price(), 0);
    }

    #[test]
    fn operand_stack_peak_applies_decreases() {
        let mut status = GasStatus::new_unmetered();
        // Push 100, pop 60, push 30, pop 70: the running size is
        // 100 → 40 → 70 → 0, so the true peak is 100.
        status.charge(1, 1, 0, 100, 0).unwrap();
        status.charge(1, 0, 1, 0, 60).unwrap();
        status.charge(1, 1, 0, 30, 0).unwrap();
        status.charge(1, 0, 1, 0, 70).unwrap();
        let profile = status.resource_profile();
        assert_eq!(profile.stack_size_high_water_mark, 100);

        // A later spike above the previous peak raises it.
        status.charge(1, 1, 0, 200, 0).unwrap();
        let profile = status.resource_profile();
        assert_eq!(profile.stack_size_high_water_mark, 200);
    }

    #[test]
    fn frame_drop_captures_in_place_growth() {
        let mut status = GasStatus::new_unmetered();
        // Outer frame receives 8 bytes of arguments, inner frame 4.
        status.record_call_frame(8);
        status.record_call_frame(4);
        assert_eq!(status.resource_profile().locals_size_high_water_mark, 12);

        // The inner frame drops 20 bytes although only 4 were recorded: the
        // missing 16 grew in place through references (e.g. vector pushes via
        // `&mut`), so they are recorded late — the high-water mark includes
        // them — and the running size returns to the outer frame's 8, never
        // below it.
        status.record_drop_frame(20);
        assert_eq!(status.resource_profile().locals_size_high_water_mark, 28);
        status.record_store_loc(1);
        assert_eq!(status.resource_profile().locals_size_high_water_mark, 28);

        status.record_drop_frame(9);
        // Store/move without any live frame (the entry function's frame gets
        // no record_call_frame) lands in an implicit root frame.
        status.record_store_loc(5);
        status.record_move_loc(5);
        assert_eq!(status.resource_profile().locals_size_high_water_mark, 28);
    }

    #[test]
    fn byte_gas_split_between_reads_and_publish() {
        let cost_table = initial_cost_schedule_v1();
        let mut status = GasStatus::new(cost_table, 1_000_000, 1, 1);
        status.charge_bytes(10, 3).unwrap();
        status.charge_publish_bytes(5, 4).unwrap();
        let profile = status.resource_profile();
        assert_eq!(profile.storage_read_gas, 30);
        assert_eq!(profile.package_publish_gas, 20);
        // Neither per-byte category leaks into the interpreter share.
        assert_eq!(profile.interpreter_gas, 0);
    }

    /// Manual benchmark of the per-instruction charging hot path, used to
    /// measure the resource-profile counters' overhead. Run with:
    /// `cargo test --release -p iota-types --lib bench_charge -- --ignored
    /// --nocapture`
    #[test]
    #[ignore = "manual benchmark, run explicitly in release mode"]
    fn bench_charge_hot_path() {
        let cost_table = initial_cost_schedule_v1();
        let mut status = GasStatus::new(cost_table, u64::MAX / 2_000, 1, 1);
        let iterations: u64 = 50_000_000;
        let start = std::time::Instant::now();
        for _ in 0..iterations {
            status.charge(1, 1, 1, 8, 8).unwrap();
        }
        let elapsed = start.elapsed();
        println!(
            "charge(): {:.2} ns/call over {iterations} calls (total {elapsed:?}, gas used {})",
            elapsed.as_nanos() as f64 / iterations as f64,
            status.gas_used_pre_gas_price(),
        );
    }

    #[test]
    fn package_loads_surface_in_resource_profile() {
        let mut status = GasStatus::new_unmetered();
        status.record_package_loads(2, 5_000);
        let profile = status.resource_profile();
        assert_eq!(profile.packages_loaded, 2);
        assert_eq!(profile.package_bytes_loaded, 5_000);
    }

    #[test]
    fn hash_input_bytes_counted_for_hashing_natives_only() {
        let cost_table = initial_cost_schedule_v1();
        let mut status = GasStatus::new(cost_table, 1_000_000, 1, 1);
        let before = status.gas_used_pre_gas_price();

        status.set_pending_native_function("0x2::hash", "keccak256");
        assert!(status.pending_native_streams_input());
        status.record_hash_input_bytes(512);
        status.set_pending_native_function("0x1::hash", "sha2_256");
        assert!(status.pending_native_streams_input());
        status.record_hash_input_bytes(256);
        status.set_pending_native_function("0x2::hmac", "hmac_sha3_256");
        assert!(status.pending_native_streams_input());

        // Non-hashing natives — including a user module named `hash` at a
        // non-system address — do not stream.
        status.set_pending_native_function("0x2::bls12381", "bls12381_min_sig_verify");
        assert!(!status.pending_native_streams_input());
        status.set_pending_native_function("0xabc::hash", "keccak256");
        assert!(!status.pending_native_streams_input());

        assert_eq!(status.gas_used_pre_gas_price(), before);
        assert_eq!(status.resource_profile().hash_input_bytes, 768);
    }
}
