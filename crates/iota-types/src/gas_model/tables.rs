// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeMap;

use move_binary_format::errors::{PartialVMError, PartialVMResult};
use move_core_types::{
    gas_algebra::{AbstractMemorySize, InternalGas},
    language_storage::ModuleId,
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

    // Counters feeding `ResourceProfile`; updating them deducts no gas.
    profile: ProfileCounters,
}

/// Counters feeding [`ResourceProfile`]; updating them deducts no gas.
#[derive(Debug, Default)]
struct ProfileCounters {
    locals_size_current: u64,
    locals_size_high_water_mark: u64,
    // Abstract bytes added to each live frame's locals, innermost frame last.
    frame_locals_added: Vec<u64>,
    // Locals high-water mark before the most recent `record_call_frame`, kept
    // until the next locals event so a native callee can discard its frame.
    locals_peak_before_call: Option<u64>,
    // Unlike the charged `GasStatus::stack_size_current`, this applies
    // decreases too.
    stack_size_current: u64,
    stack_size_peak: u64,
    native_gas_deducted: u64,
    storage_read_gas_deducted: u64,
    publish_gas_deducted: u64,
    coin_transfer_gas_deducted: u64,
    interp_instr_flow: u64,
    interp_stack_size_flow: u64,
    interp_stack_height_flow: u64,
    // Identity of the native currently executing, and its rendered key in
    // `native_by_function`; the key is rebuilt only when the identity changes.
    pending_native_module: Option<ModuleId>,
    pending_native_name: String,
    pending_native_key: String,
    native_by_function: BTreeMap<String, NativeFunctionCounters>,
    input_object_count: u64,
    input_object_bytes: u64,
    child_object_reads: u64,
    child_object_read_bytes: u64,
    object_runtime_cached_bytes: u64,
    packages_loaded: u64,
    package_bytes_loaded: u64,
    event_count: u64,
    event_bytes: u64,
}

/// Per-function entry of `ProfileCounters::native_by_function`.
#[derive(Debug, Default)]
struct NativeFunctionCounters {
    calls: u64,
    gas: u64,
    input_bytes: u64,
}

impl ProfileCounters {
    fn increase_locals_size(&mut self, amount: u64) {
        self.locals_size_current = self.locals_size_current.saturating_add(amount);
        if self.locals_size_current > self.locals_size_high_water_mark {
            self.locals_size_high_water_mark = self.locals_size_current;
        }
    }

    fn decrease_locals_size(&mut self, amount: u64) {
        self.locals_size_current = self.locals_size_current.saturating_sub(amount);
    }

    // Opens an implicit root frame if none is live, since the entry
    // function's frame gets no `record_call_frame`.
    fn add_to_current_frame(&mut self, amount: u64) {
        match self.frame_locals_added.last_mut() {
            Some(top) => *top = top.saturating_add(amount),
            None => self.frame_locals_added.push(amount),
        }
    }

    fn record_call_frame(&mut self, args_size: u64) {
        self.locals_peak_before_call = Some(self.locals_size_high_water_mark);
        self.increase_locals_size(args_size);
        self.frame_locals_added.push(args_size);
    }

    fn record_store_loc(&mut self, size: u64) {
        self.locals_peak_before_call = None;
        self.increase_locals_size(size);
        self.add_to_current_frame(size);
    }

    fn record_move_loc(&mut self, size: u64) {
        self.locals_peak_before_call = None;
        let tracked = self.frame_locals_added.last().copied().unwrap_or(0);
        if size > tracked {
            self.increase_locals_size(size - tracked);
        }
        self.decrease_locals_size(size);
        if let Some(top) = self.frame_locals_added.last_mut() {
            *top = top.saturating_sub(size);
        }
    }

    fn record_drop_frame(&mut self, dropped_size: u64) {
        self.locals_peak_before_call = None;
        let tracked = self.frame_locals_added.pop().unwrap_or(0);
        // The excess is growth in place through `&mut` references (e.g.
        // `vector::push_back`), which the store/move hooks never saw.
        if dropped_size > tracked {
            self.increase_locals_size(dropped_size - tracked);
        }
        // References stored in locals were counted by the store hook but are
        // not part of `dropped_size`, so release everything the frame tracked.
        self.decrease_locals_size(dropped_size.max(tracked));
    }

    fn discard_native_call_frame(&mut self) {
        let Some(peak_before) = self.locals_peak_before_call.take() else {
            return;
        };
        let args_size = self.frame_locals_added.pop().unwrap_or(0);
        self.decrease_locals_size(args_size);
        self.locals_size_high_water_mark = peak_before;
    }

    fn record_native_function_identity(&mut self, module_id: &ModuleId, function_name: &str) {
        if self.pending_native_module.as_ref() == Some(module_id)
            && self.pending_native_name == function_name
        {
            return;
        }
        self.pending_native_module = Some(module_id.clone());
        self.pending_native_name.clear();
        self.pending_native_name.push_str(function_name);
        self.pending_native_key.clear();
        self.pending_native_key
            .push_str(&module_id.short_str_lossless());
        self.pending_native_key.push_str("::");
        self.pending_native_key.push_str(function_name);
    }

    fn record_native_input_bytes(&mut self, bytes: u64) {
        // Look up before inserting so the key is cloned only once per function.
        if let Some(counters) = self.native_by_function.get_mut(&self.pending_native_key) {
            counters.input_bytes = counters.input_bytes.saturating_add(bytes);
        } else {
            self.native_by_function.insert(
                self.pending_native_key.clone(),
                NativeFunctionCounters {
                    input_bytes: bytes,
                    ..Default::default()
                },
            );
        }
    }

    fn record_native_charge(&mut self, deducted: u64) {
        self.native_gas_deducted = self.native_gas_deducted.saturating_add(deducted);
        if let Some(counters) = self.native_by_function.get_mut(&self.pending_native_key) {
            counters.gas = counters.gas.saturating_add(deducted);
            counters.calls = counters.calls.saturating_add(1);
        } else {
            self.native_by_function.insert(
                self.pending_native_key.clone(),
                NativeFunctionCounters {
                    calls: 1,
                    gas: deducted,
                    input_bytes: 0,
                },
            );
        }
    }
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
            profile: ProfileCounters::default(),
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
            profile: ProfileCounters::default(),
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
        // Native calls also route through here; the meter subtracts their
        // share via `discount_native_flows`.
        let profile = &mut self.profile;
        profile.interp_instr_flow = profile.interp_instr_flow.saturating_add(num_instructions);
        profile.interp_stack_size_flow = profile.interp_stack_size_flow.saturating_add(incr_size);
        profile.interp_stack_height_flow = profile.interp_stack_height_flow.saturating_add(pushes);

        // The charged `stack_size_current` intentionally ignores decreases, so
        // `decr_size` is applied only to the profile's copy.
        profile.stack_size_current = profile.stack_size_current.saturating_add(incr_size);
        if profile.stack_size_current > profile.stack_size_peak {
            profile.stack_size_peak = profile.stack_size_current;
        }
        profile.stack_size_current = profile.stack_size_current.saturating_sub(decr_size);

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
        self.charge_bytes_into(size, cost_per_byte, |profile| {
            &mut profile.storage_read_gas_deducted
        })
    }

    /// Like [`charge_bytes`](Self::charge_bytes), but records the deducted gas
    /// as package publish/upgrade gas instead of storage-read gas.
    pub fn charge_publish_bytes(&mut self, size: usize, cost_per_byte: u64) -> PartialVMResult<()> {
        self.charge_bytes_into(size, cost_per_byte, |profile| {
            &mut profile.publish_gas_deducted
        })
    }

    /// Like [`charge_bytes`](Self::charge_bytes), but records the deducted gas
    /// as coin-transfer gas instead of storage-read gas.
    pub fn charge_coin_transfer_bytes(
        &mut self,
        size: usize,
        cost_per_byte: u64,
    ) -> PartialVMResult<()> {
        self.charge_bytes_into(size, cost_per_byte, |profile| {
            &mut profile.coin_transfer_gas_deducted
        })
    }

    // Deducts `size * cost_per_byte` and adds the deducted gas to the chosen
    // profile counter.
    fn charge_bytes_into(
        &mut self,
        size: usize,
        cost_per_byte: u64,
        counter: impl FnOnce(&mut ProfileCounters) -> &mut u64,
    ) -> PartialVMResult<()> {
        let computation_cost = size as u64 * cost_per_byte;
        let gas_before = self.gas_left;
        let result = self.deduct_units(computation_cost);
        let deducted = Self::gas_delta(gas_before, self.gas_left);
        let counter = counter(&mut self.profile);
        *counter = counter.saturating_add(deducted);
        result
    }

    /// `before - after`, saturating at zero.
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

    /// Record a function call whose arguments total `args_size` abstract
    /// bytes. The interpreter charges a call before it knows whether the
    /// callee is native, so natives pass through here too; see
    /// [`discard_native_call_frame`](Self::discard_native_call_frame).
    pub fn record_call_frame(&mut self, args_size: u64) {
        self.profile.record_call_frame(args_size);
    }

    /// Record a value stored into a local. Storing over an occupied local
    /// over-counts, since the displaced value is not visible here.
    pub fn record_store_loc(&mut self, size: u64) {
        self.profile.record_store_loc(size);
    }

    /// Record a value moved out of a local. A value larger than the frame has
    /// tracked was an entry-function argument or grew in place through `&mut`;
    /// the excess is counted before release so the peak sees it.
    pub fn record_move_loc(&mut self, size: u64) {
        self.profile.record_move_loc(size);
    }

    /// Record a frame drop, where `dropped_size` is the total abstract size of
    /// the non-reference values still in the frame's locals.
    pub fn record_drop_frame(&mut self, dropped_size: u64) {
        self.profile.record_drop_frame(dropped_size);
    }

    /// Undo the frame pushed by [`record_call_frame`](Self::record_call_frame)
    /// for a native callee, which the interpreter never drops. A no-op unless
    /// that push was the last locals event, so a native invoked without a
    /// preceding call leaves the caller's frames alone.
    pub fn discard_native_call_frame(&mut self) {
        self.profile.discard_native_call_frame();
    }

    pub fn record_package_loads(&mut self, count: u64, bytes: u64) {
        self.profile.packages_loaded = count;
        self.profile.package_bytes_loaded = bytes;
    }

    /// Set the native function that the following native recordings are
    /// attributed to. The per-function key is rendered only when the identity
    /// differs from the previous call.
    pub fn record_native_function_identity(&mut self, module_id: &ModuleId, function_name: &str) {
        self.profile
            .record_native_function_identity(module_id, function_name);
    }

    /// Record the abstract size of the pending native's arguments.
    pub fn record_native_input_bytes(&mut self, bytes: u64) {
        self.profile.record_native_input_bytes(bytes);
    }

    /// Record the gas deducted since `gas_left_before` against the pending
    /// native function, and count the call. Does not charge gas.
    pub fn record_native_gas_deducted(&mut self, gas_left_before: InternalGas) {
        let deducted = Self::gas_delta(gas_left_before, self.gas_left);
        self.profile.record_native_charge(deducted);
    }

    /// Undo a native call's additions to the interpreter counters. Pass the
    /// same values the native's [`charge`](Self::charge) call used.
    pub fn discount_native_flows(&mut self, num_instructions: u64, pushes: u64, incr_size: u64) {
        let profile = &mut self.profile;
        profile.interp_instr_flow = profile.interp_instr_flow.saturating_sub(num_instructions);
        profile.interp_stack_size_flow = profile.interp_stack_size_flow.saturating_sub(incr_size);
        profile.interp_stack_height_flow = profile.interp_stack_height_flow.saturating_sub(pushes);
    }

    pub fn record_input_objects(&mut self, count: u64, bytes: u64) {
        self.profile.input_object_count = count;
        self.profile.input_object_bytes = bytes;
    }

    pub fn record_object_runtime_usage(&mut self, reads: u64, read_bytes: u64, cached_bytes: u64) {
        self.profile.child_object_reads = reads;
        self.profile.child_object_read_bytes = read_bytes;
        self.profile.object_runtime_cached_bytes = cached_bytes;
    }

    pub fn record_events(&mut self, count: u64, bytes: u64) {
        self.profile.event_count = count;
        self.profile.event_bytes = bytes;
    }

    /// The [`ResourceProfile`] without the write fields (written and deleted
    /// objects), which the caller fills in from storage tracking.
    pub fn resource_profile(&self) -> ResourceProfile {
        let profile = &self.profile;
        let total_deducted = Self::gas_delta(self.initial_budget, self.gas_left);
        let interpreter_gas = total_deducted
            .saturating_sub(profile.native_gas_deducted)
            .saturating_sub(profile.storage_read_gas_deducted)
            .saturating_sub(profile.publish_gas_deducted)
            .saturating_sub(profile.coin_transfer_gas_deducted);
        let mut native_gas_by_function = BTreeMap::new();
        let mut native_calls_by_function = BTreeMap::new();
        let mut native_input_bytes_by_function = BTreeMap::new();
        for (function, counters) in &profile.native_by_function {
            native_input_bytes_by_function.insert(function.clone(), counters.input_bytes);
            // A function whose charge never completed has no call to report.
            if counters.calls > 0 {
                native_gas_by_function.insert(function.clone(), counters.gas);
                native_calls_by_function.insert(function.clone(), counters.calls);
            }
        }
        ResourceProfile {
            instructions_executed: self.instructions_executed,
            num_native_calls: self.num_native_calls,
            interpreter_gas,
            interp_instruction_count: profile.interp_instr_flow,
            interp_stack_size_flow: profile.interp_stack_size_flow,
            interp_stack_height_flow: profile.interp_stack_height_flow,
            native_gas: profile.native_gas_deducted,
            native_gas_by_function,
            native_calls_by_function,
            storage_read_gas: profile.storage_read_gas_deducted,
            package_publish_gas: profile.publish_gas_deducted,
            coin_transfer_gas: profile.coin_transfer_gas_deducted,
            computation_gas_used: self.gas_used_pre_gas_price(),
            stack_size_high_water_mark: profile.stack_size_peak,
            stack_height_high_water_mark: self.stack_height_high_water_mark,
            locals_size_high_water_mark: profile.locals_size_high_water_mark,
            object_runtime_cached_bytes: profile.object_runtime_cached_bytes,
            input_object_count: profile.input_object_count,
            input_object_bytes: profile.input_object_bytes,
            child_object_reads: profile.child_object_reads,
            packages_loaded: profile.packages_loaded,
            package_bytes_loaded: profile.package_bytes_loaded,
            child_object_read_bytes: profile.child_object_read_bytes,
            written_object_count: 0,
            written_bytes: 0,
            deleted_object_count: 0,
            event_count: profile.event_count,
            event_bytes: profile.event_bytes,
            native_input_bytes_by_function,
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
    use move_core_types::{account_address::AccountAddress, identifier::Identifier};

    use super::*;

    fn native_id(address: AccountAddress, module: &str) -> ModuleId {
        ModuleId::new(address, Identifier::new(module).unwrap())
    }

    #[test]
    fn locals_size_tracking_records_high_water_mark() {
        let mut status = GasStatus::new_unmetered();
        status.profile.increase_locals_size(100);
        status.profile.increase_locals_size(50);
        status.profile.decrease_locals_size(120);
        status.profile.increase_locals_size(30);
        assert_eq!(status.resource_profile().locals_size_high_water_mark, 150);

        // Draining below zero saturates instead of wrapping.
        status.profile.decrease_locals_size(u64::MAX);
        status.profile.increase_locals_size(10);
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
        assert_eq!(profile.written_object_count, 0);
        assert_eq!(profile.written_bytes, 0);
        assert_eq!(profile.deleted_object_count, 0);
    }

    #[test]
    fn gas_split_into_interpreter_native_and_byte() {
        let cost_table = initial_cost_schedule_v1();
        let mut status = GasStatus::new(cost_table, 1_000_000, 1, 1);

        let before_interp = u64::from(status.gas_left);
        status.charge(1, 1, 0, 0, 0).unwrap();
        let interp = before_interp - u64::from(status.gas_left);
        assert!(interp > 0, "interpreter charge should deduct gas");

        status.record_native_function_identity(
            &native_id(AccountAddress::TWO, "ed25519"),
            "ed25519_verify",
        );
        let before_native = status.gas_left;
        status.deduct_gas(InternalGas::new(5000)).unwrap();
        status.record_native_gas_deducted(before_native);

        status.record_native_function_identity(
            &native_id(AccountAddress::TWO, "bls12381"),
            "bls12381_min_sig_verify",
        );
        let before_native2 = status.gas_left;
        status.deduct_gas(InternalGas::new(2000)).unwrap();
        status.record_native_gas_deducted(before_native2);

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
        assert_eq!(profile.interpreter_gas, interp);
    }

    #[test]
    fn interpreter_component_flows_exclude_native() {
        let cost_table = initial_cost_schedule_v1();
        let mut status = GasStatus::new(cost_table, 1_000_000, 1, 1);

        status.charge(2, 3, 0, 40, 0).unwrap();
        status.charge(1, 0, 0, 8, 0).unwrap();

        // An above-threshold native call.
        status.charge(500, 2, 0, 16, 0).unwrap();
        status.discount_native_flows(500, 2, 16);

        let profile = status.resource_profile();
        assert_eq!(profile.interp_instruction_count, 3);
        assert_eq!(profile.interp_stack_height_flow, 3);
        assert_eq!(profile.interp_stack_size_flow, 48);
    }

    #[test]
    fn measurement_counters_do_not_affect_charging() {
        let cost_table = initial_cost_schedule_v1();
        let mut status = GasStatus::new(cost_table, 1_000_000, 1, 1);
        let before = status.remaining_gas();
        status.profile.increase_locals_size(1_000_000);
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
        // Running size: 100 → 40 → 70 → 0.
        status.charge(1, 1, 0, 100, 0).unwrap();
        status.charge(1, 0, 1, 0, 60).unwrap();
        status.charge(1, 1, 0, 30, 0).unwrap();
        status.charge(1, 0, 1, 0, 70).unwrap();
        let profile = status.resource_profile();
        assert_eq!(profile.stack_size_high_water_mark, 100);

        status.charge(1, 1, 0, 200, 0).unwrap();
        let profile = status.resource_profile();
        assert_eq!(profile.stack_size_high_water_mark, 200);
    }

    #[test]
    fn frame_drop_captures_in_place_growth() {
        let mut status = GasStatus::new_unmetered();
        status.record_call_frame(8);
        status.record_call_frame(4);
        assert_eq!(status.resource_profile().locals_size_high_water_mark, 12);

        // The inner frame grew by 16 bytes in place; the running size returns
        // to the outer frame's 8.
        status.record_drop_frame(20);
        assert_eq!(status.resource_profile().locals_size_high_water_mark, 28);
        status.record_store_loc(1);
        assert_eq!(status.resource_profile().locals_size_high_water_mark, 28);

        status.record_drop_frame(9);
        // No live frame: lands in the implicit root frame.
        status.record_store_loc(5);
        status.record_move_loc(5);
        assert_eq!(status.resource_profile().locals_size_high_water_mark, 28);
    }

    #[test]
    fn moving_an_untracked_local_counts_it_before_release() {
        let mut status = GasStatus::new_unmetered();
        status.record_call_frame(100);
        status.record_call_frame(0);
        // The callee moves out a value the store hook never saw: an entry
        // argument, or one grown in place through `&mut`.
        status.record_move_loc(64);
        assert_eq!(status.resource_profile().locals_size_high_water_mark, 164);
        // The outer frame's locals are still live.
        assert_eq!(status.profile.locals_size_current, 100);

        status.record_drop_frame(0);
        status.record_drop_frame(100);
        assert_eq!(status.profile.locals_size_current, 0);
    }

    #[test]
    fn frame_drop_releases_stored_references() {
        let mut status = GasStatus::new_unmetered();
        for _ in 0..1000 {
            status.record_call_frame(0);
            // `StLoc` of a reference, counted at its reference size.
            status.record_store_loc(8);
            // The interpreter passes only non-reference locals to the drop.
            status.record_drop_frame(0);
        }
        assert_eq!(status.profile.locals_size_current, 0);
        assert_eq!(status.resource_profile().locals_size_high_water_mark, 8);
    }

    #[test]
    fn native_call_leaves_no_frame_behind() {
        let mut status = GasStatus::new_unmetered();
        status.record_store_loc(100);
        for _ in 0..10_000 {
            status.record_call_frame(40);
            status.discard_native_call_frame();
        }
        assert_eq!(status.resource_profile().locals_size_high_water_mark, 100);
        assert_eq!(status.profile.frame_locals_added, vec![100]);

        // The caller's later stores and its own drop still land in its frame.
        status.record_store_loc(10);
        status.record_drop_frame(110);
        assert_eq!(status.profile.locals_size_current, 0);
        assert!(status.profile.frame_locals_added.is_empty());
        assert_eq!(status.resource_profile().locals_size_high_water_mark, 110);
    }

    #[test]
    fn discarding_without_a_pending_call_frame_is_a_no_op() {
        let mut status = GasStatus::new_unmetered();
        // A native invoked as the entry function has no preceding call.
        status.discard_native_call_frame();
        assert!(status.profile.frame_locals_added.is_empty());

        status.record_call_frame(8);
        status.record_store_loc(4);
        // A store in the callee means the frame belongs to a Move function.
        status.discard_native_call_frame();
        assert_eq!(status.profile.frame_locals_added, vec![12]);
        assert_eq!(status.profile.locals_size_current, 12);
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
        assert_eq!(profile.interpreter_gas, 0);
    }

    /// Run with: `cargo test --release -p iota-types --lib bench_charge --
    /// --ignored --nocapture`
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
    fn native_input_bytes_attributed_per_function() {
        let cost_table = initial_cost_schedule_v1();
        let mut status = GasStatus::new(cost_table, 1_000_000, 1, 1);
        let before = status.gas_used_pre_gas_price();

        status
            .record_native_function_identity(&native_id(AccountAddress::TWO, "hash"), "keccak256");
        status.record_native_input_bytes(512);
        status.record_native_input_bytes(512);
        status.record_native_function_identity(&native_id(AccountAddress::ONE, "hash"), "sha2_256");
        status.record_native_input_bytes(256);
        status.record_native_function_identity(
            &native_id(AccountAddress::TWO, "tx_context"),
            "fresh_id",
        );
        status.record_native_input_bytes(0);

        assert_eq!(status.gas_used_pre_gas_price(), before);
        let profile = status.resource_profile();
        assert_eq!(
            profile
                .native_input_bytes_by_function
                .get("0x2::hash::keccak256"),
            Some(&1024)
        );
        assert_eq!(
            profile
                .native_input_bytes_by_function
                .get("0x1::hash::sha2_256"),
            Some(&256)
        );
        assert_eq!(
            profile
                .native_input_bytes_by_function
                .get("0x2::tx_context::fresh_id"),
            Some(&0)
        );
    }

    #[test]
    fn coin_transfer_gas_attributed_separately() {
        let cost_table = initial_cost_schedule_v1();
        let mut status = GasStatus::new(cost_table, 1_000_000, 1, 1);
        status.charge_bytes(10, 3).unwrap();
        status.charge_coin_transfer_bytes(5, 4).unwrap();
        let profile = status.resource_profile();
        assert_eq!(profile.storage_read_gas, 30);
        assert_eq!(profile.coin_transfer_gas, 20);
        assert_eq!(profile.interpreter_gas, 0);
    }

    #[test]
    fn native_identity_switches_rebuild_the_key() {
        let mut status = GasStatus::new_unmetered();
        let hash = native_id(AccountAddress::TWO, "hash");
        let tx_context = native_id(AccountAddress::TWO, "tx_context");

        status.record_native_function_identity(&hash, "keccak256");
        status.record_native_input_bytes(5);
        // Same identity again: the cached key must still attribute correctly.
        status.record_native_function_identity(&hash, "keccak256");
        status.record_native_input_bytes(7);
        // Same module, different function.
        status.record_native_function_identity(&hash, "blake2b256");
        status.record_native_input_bytes(1);
        status.record_native_function_identity(&tx_context, "fresh_id");
        status.record_native_input_bytes(2);
        // Back to the first identity.
        status.record_native_function_identity(&hash, "keccak256");
        status.record_native_input_bytes(3);

        let profile = status.resource_profile();
        assert_eq!(
            profile
                .native_input_bytes_by_function
                .get("0x2::hash::keccak256"),
            Some(&15)
        );
        assert_eq!(
            profile
                .native_input_bytes_by_function
                .get("0x2::hash::blake2b256"),
            Some(&1)
        );
        assert_eq!(
            profile
                .native_input_bytes_by_function
                .get("0x2::tx_context::fresh_id"),
            Some(&2)
        );
        // Input recorded but no charge completed: no call to report.
        assert!(profile.native_calls_by_function.is_empty());
        assert!(profile.native_gas_by_function.is_empty());
    }
}
