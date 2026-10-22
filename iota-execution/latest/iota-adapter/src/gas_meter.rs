// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_move_natives::resource_profile::abstract_input_size;
use iota_types::gas_model::{
    gas_predicates::native_function_threshold_exceeded,
    tables::{GasStatus, REFERENCE_SIZE, STRUCT_SIZE, VEC_SIZE},
};
use move_binary_format::errors::PartialVMResult;
use move_core_types::{
    account_address::AccountAddress,
    gas_algebra::{AbstractMemorySize, InternalGas, NumArgs, NumBytes},
    language_storage::ModuleId,
};
use move_vm_profiler::GasProfiler;
use move_vm_types::{
    gas::{GasMeter, SimpleInstruction},
    loaded_data::runtime_types::Type,
    views::{TypeView, ValueView, ValueVisitor},
};

pub struct IotaGasMeter<'g>(pub &'g mut GasStatus);

/// A native whose execution cost depends on a closed-set selector argument
/// (an enum passed as `u8`). Such a native gets one profile counter per
/// selector value instead of one shared counter, so the fit can price each
/// variant separately.
struct NativeSelector {
    first_arg: usize,
    second_arg: Option<usize>,
    render: fn(u8, Option<u8>) -> Option<&'static str>,
}

fn render_groth16_curve(value: u8, _: Option<u8>) -> Option<&'static str> {
    // `0x2::groth16::Curve` ids.
    match value {
        0 => Some("bls12381"),
        1 => Some("bn254"),
        _ => None,
    }
}

fn group_ops_group_name(value: u8) -> Option<&'static str> {
    // `Groups::from_u8` in iota-move-natives group_ops.
    match value {
        0 => Some("bls12381_scalar"),
        1 => Some("bls12381_g1"),
        2 => Some("bls12381_g2"),
        3 => Some("bls12381_gt"),
        4 => Some("bls12381_uncompressed_g1"),
        _ => None,
    }
}

fn render_group(value: u8, _: Option<u8>) -> Option<&'static str> {
    group_ops_group_name(value)
}

fn render_group_convert(from: u8, to: Option<u8>) -> Option<&'static str> {
    match (from, to?) {
        (1, 4) => Some("bls12381_g1_to_uncompressed_g1"),
        (4, 1) => Some("bls12381_uncompressed_g1_to_g1"),
        _ => None,
    }
}

fn render_ecdsa_hash(value: u8, _: Option<u8>) -> Option<&'static str> {
    match value {
        0 => Some("keccak256"),
        1 => Some("sha256"),
        _ => None,
    }
}

/// The registry of selector-refined natives. Anything not listed keeps its
/// one-counter-per-function key.
fn native_selector(module_id: &ModuleId, function: &str) -> Option<NativeSelector> {
    if *module_id.address() != AccountAddress::TWO {
        return None;
    }
    let single = |first_arg, render| {
        Some(NativeSelector {
            first_arg,
            second_arg: None,
            render,
        })
    };
    match (module_id.name().as_str(), function) {
        ("groth16", "prepare_verifying_key_internal" | "verify_groth16_proof_internal") => {
            single(0, render_groth16_curve)
        }
        (
            "group_ops",
            "internal_validate"
            | "internal_add"
            | "internal_sub"
            | "internal_mul"
            | "internal_div"
            | "internal_hash_to"
            | "internal_multi_scalar_mul"
            | "internal_pairing"
            | "internal_sum",
        ) => single(0, render_group),
        ("group_ops", "internal_convert") => Some(NativeSelector {
            first_arg: 0,
            second_arg: Some(1),
            render: render_group_convert,
        }),
        ("ecdsa_k1", "secp256k1_ecrecover") | ("ecdsa_r1", "secp256r1_ecrecover") => {
            single(2, render_ecdsa_hash)
        }
        ("ecdsa_k1", "secp256k1_verify") | ("ecdsa_r1", "secp256r1_verify") => {
            single(3, render_ecdsa_hash)
        }
        _ => None,
    }
}

/// Captures a top-level `u8` argument's value; containers and references are
/// not descended into, so only a genuine scalar selector is captured.
#[derive(Default)]
struct SelectorProbe {
    value: Option<u8>,
}

impl ValueVisitor for SelectorProbe {
    fn visit_u8(&mut self, depth: usize, val: u8) {
        if depth == 0 {
            self.value = Some(val);
        }
    }
    fn visit_u16(&mut self, _depth: usize, _val: u16) {}
    fn visit_u32(&mut self, _depth: usize, _val: u32) {}
    fn visit_u64(&mut self, _depth: usize, _val: u64) {}
    fn visit_u128(&mut self, _depth: usize, _val: u128) {}
    fn visit_u256(&mut self, _depth: usize, _val: move_core_types::u256::U256) {}
    fn visit_bool(&mut self, _depth: usize, _val: bool) {}
    fn visit_address(&mut self, _depth: usize, _val: AccountAddress) {}
    fn visit_struct(&mut self, _depth: usize, _len: usize) -> bool {
        false
    }
    fn visit_variant(&mut self, _depth: usize, _len: usize) -> bool {
        false
    }
    fn visit_vec(&mut self, _depth: usize, _len: usize) -> bool {
        false
    }
    fn visit_ref(&mut self, _depth: usize, _is_global: bool) -> bool {
        false
    }
}

/// Returns a tuple of (<pops>, <pushes>, <stack_size_decrease>,
/// <stack_size_increase>)
fn get_simple_instruction_stack_change(
    instr: SimpleInstruction,
) -> (u64, u64, AbstractMemorySize, AbstractMemorySize) {
    use SimpleInstruction::*;

    match instr {
        // NB: The `Ret` pops are accounted for in `Call` instructions, so we say `Ret` has no pops.
        Nop | Ret => (0, 0, 0.into(), 0.into()),
        BrTrue | BrFalse => (1, 0, Type::Bool.size(), 0.into()),
        Branch => (0, 0, 0.into(), 0.into()),
        LdU8 => (0, 1, 0.into(), Type::U8.size()),
        LdU16 => (0, 1, 0.into(), Type::U16.size()),
        LdU32 => (0, 1, 0.into(), Type::U32.size()),
        LdU64 => (0, 1, 0.into(), Type::U64.size()),
        LdU128 => (0, 1, 0.into(), Type::U128.size()),
        LdU256 => (0, 1, 0.into(), Type::U256.size()),
        LdTrue | LdFalse => (0, 1, 0.into(), Type::Bool.size()),
        FreezeRef => (1, 1, REFERENCE_SIZE, REFERENCE_SIZE),
        ImmBorrowLoc | MutBorrowLoc => (0, 1, 0.into(), REFERENCE_SIZE),
        ImmBorrowField | MutBorrowField | ImmBorrowFieldGeneric | MutBorrowFieldGeneric => {
            (1, 1, REFERENCE_SIZE, REFERENCE_SIZE)
        }
        // Since we don't have the size of the value being cast here we take a conservative
        // over-approximation: it is _always_ getting cast from the smallest integer type.
        CastU8 => (1, 1, Type::U8.size(), Type::U8.size()),
        CastU16 => (1, 1, Type::U8.size(), Type::U16.size()),
        CastU32 => (1, 1, Type::U8.size(), Type::U32.size()),
        CastU64 => (1, 1, Type::U8.size(), Type::U64.size()),
        CastU128 => (1, 1, Type::U8.size(), Type::U128.size()),
        CastU256 => (1, 1, Type::U8.size(), Type::U256.size()),
        // NB: We don't know the size of what integers we're dealing with, so we conservatively
        // over-approximate by popping the smallest integers, and push the largest.
        Add | Sub | Mul | Mod | Div => (2, 1, Type::U8.size() + Type::U8.size(), Type::U256.size()),
        BitOr | BitAnd | Xor => (2, 1, Type::U8.size() + Type::U8.size(), Type::U256.size()),
        Shl | Shr => (2, 1, Type::U8.size() + Type::U8.size(), Type::U256.size()),
        Or | And => (
            2,
            1,
            Type::Bool.size() + Type::Bool.size(),
            Type::Bool.size(),
        ),
        Lt | Gt | Le | Ge => (2, 1, Type::U8.size() + Type::U8.size(), Type::Bool.size()),
        Not => (1, 1, Type::Bool.size(), Type::Bool.size()),
        Abort => (1, 0, Type::U64.size(), 0.into()),
    }
}

impl GasMeter for IotaGasMeter<'_> {
    /// Charge an instruction and fail if not enough gas units are left.
    fn charge_simple_instr(&mut self, instr: SimpleInstruction) -> PartialVMResult<()> {
        let (pops, pushes, pop_size, push_size) = get_simple_instruction_stack_change(instr);
        self.0
            .charge(1, pushes, pops, push_size.into(), pop_size.into())
    }

    fn charge_pop(&mut self, popped_val: impl ValueView) -> PartialVMResult<()> {
        self.0
            .charge(1, 0, 1, 0, abstract_memory_size(popped_val).into())
    }

    fn charge_native_function(
        &mut self,
        amount: InternalGas,
        ret_vals: Option<impl ExactSizeIterator<Item = impl ValueView>>,
    ) -> PartialVMResult<()> {
        // Charge for the number of pushes on to the stack that the return of this
        // function is going to cause.
        let pushes = ret_vals
            .as_ref()
            .map(|ret_vals| ret_vals.len())
            .unwrap_or(0) as u64;
        // Calculate the number of bytes that are getting pushed onto the stack.
        let size_increase = ret_vals
            .map(|ret_vals| {
                ret_vals.fold(AbstractMemorySize::zero(), |acc, elem| {
                    acc + abstract_memory_size(elem)
                })
            })
            .unwrap_or_else(AbstractMemorySize::zero);
        self.0.record_native_call();
        let gas_before = self.0.gas_left;
        let threshold_exceeded =
            native_function_threshold_exceeded(self.0.gas_model_version, self.0.num_native_calls);
        let result = if threshold_exceeded {
            // Charge for the stack operations. We don't count this as an "instruction"
            // since we already accounted for the `Call` instruction in the
            // `charge_native_function_before_execution` call.
            // The amount returned by the native function is viewed as the "virtual"
            // instruction cost for the native function, and will be charged and
            // contribute to the overall cost tier of the transaction
            // accordingly.
            self.0
                .charge(amount.into(), pushes, 0, size_increase.into(), 0)
        } else {
            // Charge for the stack operations. We don't count this as an "instruction"
            // since we already accounted for the `Call` instruction in the
            // `charge_native_function_before_execution` call.
            self.0
                .charge(0, pushes, 0, size_increase.into(), 0)
                // Now charge the gas that the native function told us to charge.
                .and_then(|()| self.0.deduct_gas(amount))
        };
        self.0.record_native_gas_deducted(gas_before);
        let native_instructions = if threshold_exceeded { amount.into() } else { 0 };
        self.0
            .discount_native_flows(native_instructions, pushes, size_increase.into());
        result
    }

    fn charge_native_function_before_execution(
        &mut self,
        _ty_args: impl ExactSizeIterator<Item = impl TypeView>,
        args: impl ExactSizeIterator<Item = impl ValueView>,
    ) -> PartialVMResult<()> {
        // Determine the number of pops that are going to be needed for this function
        // call, and charge for them.
        let pops = args.len() as u64;
        let selector = self
            .0
            .pending_native_identity()
            .and_then(|(module_id, function)| native_selector(module_id, function));
        let mut selector_values: (Option<u8>, Option<u8>) = (None, None);
        let mut input_bytes = AbstractMemorySize::zero();
        // Calculate the size decrease of the stack from the above pops.
        let mut stack_reduction_size = AbstractMemorySize::new(pops);
        for (index, elem) in args.enumerate() {
            if let Some(spec) = &selector {
                if index == spec.first_arg || Some(index) == spec.second_arg {
                    let mut probe = SelectorProbe::default();
                    elem.visit(&mut probe);
                    if index == spec.first_arg {
                        selector_values.0 = probe.value;
                    } else {
                        selector_values.1 = probe.value;
                    }
                }
            }
            input_bytes += abstract_input_size(&elem);
            stack_reduction_size += abstract_memory_size(elem);
        }
        if let Some(spec) = selector {
            if let Some(value) = selector_values.0 {
                if let Some(variant) = (spec.render)(value, selector_values.1) {
                    self.0.refine_native_function_identity(variant);
                }
            }
        }
        self.0.discard_native_call_frame();
        self.0.record_native_input_bytes(input_bytes.into());
        // Track that this is going to be popping from the operand stack. We also
        // increment the instruction count as we need to account for the `Call`
        // bytecode that initiated this native call.
        self.0.charge(1, 0, pops, 0, stack_reduction_size.into())
    }

    fn record_native_function_identity(&mut self, module_id: &ModuleId, function_name: &str) {
        self.0
            .record_native_function_identity(module_id, function_name);
    }

    fn charge_call(
        &mut self,
        _module_id: &ModuleId,
        _func_name: &str,
        args: impl ExactSizeIterator<Item = impl ValueView>,
        _num_locals: NumArgs,
    ) -> PartialVMResult<()> {
        // We will have to perform this many pops for the call.
        let pops = args.len() as u64;
        // Size stays the same -- we're just moving it from the operand stack to the
        // locals. But the size on the operand stack is reduced by sum_{args}
        // arg.size().
        let stack_reduction_size = args.fold(AbstractMemorySize::new(0), |acc, elem| {
            acc + abstract_memory_size(elem)
        });
        self.0.record_call_frame(stack_reduction_size.into());
        self.0.charge(1, 0, pops, 0, stack_reduction_size.into())
    }

    fn charge_call_generic(
        &mut self,
        _module_id: &ModuleId,
        _func_name: &str,
        _ty_args: impl ExactSizeIterator<Item = impl TypeView>,
        args: impl ExactSizeIterator<Item = impl ValueView>,
        _num_locals: NumArgs,
    ) -> PartialVMResult<()> {
        // We have to perform this many pops from the operand stack for this function
        // call.
        let pops = args.len() as u64;
        // Calculate the size reduction on the operand stack.
        let stack_reduction_size = args.fold(AbstractMemorySize::new(0), |acc, elem| {
            acc + abstract_memory_size(elem)
        });
        self.0.record_call_frame(stack_reduction_size.into());
        // Charge for the pops, no pushes, and account for the stack size decrease. Also
        // track the `CallGeneric` instruction we must have encountered for
        // this.
        self.0.charge(1, 0, pops, 0, stack_reduction_size.into())
    }

    fn charge_ld_const(&mut self, size: NumBytes) -> PartialVMResult<()> {
        // Charge for the load from the locals onto the stack.
        self.0.charge(1, 1, 0, u64::from(size), 0)
    }

    fn charge_ld_const_after_deserialization(
        &mut self,
        _val: impl ValueView,
    ) -> PartialVMResult<()> {
        // We already charged for this based on the bytes that we're loading so don't
        // charge again.
        Ok(())
    }

    fn charge_copy_loc(&mut self, val: impl ValueView) -> PartialVMResult<()> {
        // Charge for the copy of the local onto the stack.
        self.0.charge(1, 1, 0, abstract_memory_size(val).into(), 0)
    }

    fn charge_move_loc(&mut self, val: impl ValueView) -> PartialVMResult<()> {
        // Charge for the move of the local on to the stack.
        let size = abstract_memory_size(val);
        self.0.record_move_loc(size.into());
        self.0.charge(1, 1, 0, size.into(), 0)
    }

    fn charge_store_loc(&mut self, val: impl ValueView) -> PartialVMResult<()> {
        // Charge for the storing of the value on the stack into a local.
        let size = abstract_memory_size(val);
        self.0.record_store_loc(size.into());
        self.0.charge(1, 0, 1, 0, size.into())
    }

    fn charge_pack(
        &mut self,
        _is_generic: bool,
        args: impl ExactSizeIterator<Item = impl ValueView>,
    ) -> PartialVMResult<()> {
        // We perform `num_fields` number of pops.
        let num_fields = args.len() as u64;
        // The actual amount of memory on the stack is staying the same with the
        // addition of some extra size for the struct, so the size doesn't
        // really change much.
        self.0.charge(1, 1, num_fields, STRUCT_SIZE.into(), 0)
    }

    fn charge_unpack(
        &mut self,
        _is_generic: bool,
        args: impl ExactSizeIterator<Item = impl ValueView>,
    ) -> PartialVMResult<()> {
        // We perform `num_fields` number of pushes.
        let num_fields = args.len() as u64;
        self.0.charge(1, num_fields, 1, 0, STRUCT_SIZE.into())
    }

    fn charge_variant_switch(&mut self, val: impl ValueView) -> PartialVMResult<()> {
        // We perform a single pop of a value from the stack.
        self.0.charge(1, 0, 1, 0, abstract_memory_size(val).into())
    }

    fn charge_read_ref(&mut self, ref_val: impl ValueView) -> PartialVMResult<()> {
        // We read the reference so we are decreasing the size of the stack by the size
        // of the reference, and adding to it the size of the value that has
        // been read from that reference.
        self.0.charge(
            1,
            1,
            1,
            abstract_memory_size(ref_val).into(),
            REFERENCE_SIZE.into(),
        )
    }

    fn charge_write_ref(
        &mut self,
        new_val: impl ValueView,
        old_val: impl ValueView,
    ) -> PartialVMResult<()> {
        // TODO(tzakian): We should account for this elsewhere as the owner of data the
        // the reference points to won't be on the stack. For now though, we
        // treat it as adding to the stack size.
        self.0.charge(
            1,
            1,
            2,
            abstract_memory_size(new_val).into(),
            abstract_memory_size(old_val).into(),
        )
    }

    fn charge_eq(&mut self, lhs: impl ValueView, rhs: impl ValueView) -> PartialVMResult<()> {
        let size_reduction = abstract_memory_size(lhs) + abstract_memory_size(rhs);
        self.0.charge(
            1,
            1,
            2,
            (Type::Bool.size() + size_reduction).into(),
            size_reduction.into(),
        )
    }

    fn charge_neq(&mut self, lhs: impl ValueView, rhs: impl ValueView) -> PartialVMResult<()> {
        let size_reduction = abstract_memory_size(lhs) + abstract_memory_size(rhs);
        self.0
            .charge(1, 1, 2, Type::Bool.size().into(), size_reduction.into())
    }

    fn charge_vec_pack<'a>(
        &mut self,
        _ty: impl TypeView + 'a,
        args: impl ExactSizeIterator<Item = impl ValueView>,
    ) -> PartialVMResult<()> {
        // We will perform `num_args` number of pops.
        let num_args = args.len() as u64;
        // The amount of data on the stack stays constant except we have some extra
        // metadata for the vector to hold the length of the vector.
        self.0.charge(1, 1, num_args, VEC_SIZE.into(), 0)
    }

    fn charge_vec_len(&mut self, _ty: impl TypeView) -> PartialVMResult<()> {
        self.0
            .charge(1, 1, 1, Type::U64.size().into(), REFERENCE_SIZE.into())
    }

    fn charge_vec_borrow(
        &mut self,
        _is_mut: bool,
        _ty: impl TypeView,
        _is_success: bool,
    ) -> PartialVMResult<()> {
        self.0.charge(
            1,
            1,
            2,
            REFERENCE_SIZE.into(),
            (REFERENCE_SIZE + Type::U64.size()).into(),
        )
    }

    fn charge_vec_push_back(
        &mut self,
        _ty: impl TypeView,
        _val: impl ValueView,
    ) -> PartialVMResult<()> {
        // The value was already on the stack, so we aren't increasing the number of
        // bytes on the stack.
        self.0.charge(1, 0, 2, 0, REFERENCE_SIZE.into())
    }

    fn charge_vec_pop_back(
        &mut self,
        _ty: impl TypeView,
        _val: Option<impl ValueView>,
    ) -> PartialVMResult<()> {
        self.0.charge(1, 1, 1, 0, REFERENCE_SIZE.into())
    }

    fn charge_vec_unpack(
        &mut self,
        _ty: impl TypeView,
        expect_num_elements: NumArgs,
        _elems: impl ExactSizeIterator<Item = impl ValueView>,
    ) -> PartialVMResult<()> {
        // Charge for the pushes
        let pushes = u64::from(expect_num_elements);
        // The stack size stays pretty much the same modulo the additional vector size
        self.0.charge(1, pushes, 1, 0, VEC_SIZE.into())
    }

    fn charge_vec_swap(&mut self, _ty: impl TypeView) -> PartialVMResult<()> {
        let size_decrease = REFERENCE_SIZE + Type::U64.size() + Type::U64.size();
        self.0.charge(1, 1, 1, 0, size_decrease.into())
    }

    fn charge_drop_frame(
        &mut self,
        locals: impl Iterator<Item = impl ValueView>,
    ) -> PartialVMResult<()> {
        let dropped = locals.fold(AbstractMemorySize::zero(), |acc, val| {
            acc + abstract_memory_size(val)
        });
        self.0.record_drop_frame(dropped.into());
        Ok(())
    }

    fn remaining_gas(&self) -> InternalGas {
        if !self.0.charge {
            return InternalGas::new(u64::MAX);
        }
        self.0.gas_left
    }

    fn get_profiler_mut(&mut self) -> Option<&mut GasProfiler> {
        self.0.profiler.as_mut()
    }

    fn set_profiler(&mut self, profiler: GasProfiler) {
        self.0.profiler = Some(profiler);
    }
}

fn abstract_memory_size(val: impl ValueView) -> AbstractMemorySize {
    val.abstract_memory_size(false)
}

#[cfg(test)]
mod tests {
    use iota_types::gas_model::tables::initial_cost_schedule_v1;
    use move_core_types::{
        account_address::AccountAddress, identifier::Identifier, language_storage::TypeTag,
    };
    use move_vm_types::values::Value;

    use super::*;

    struct NoTypeArgs;
    impl TypeView for NoTypeArgs {
        fn to_type_tag(&self) -> TypeTag {
            unreachable!("the benchmarks pass no type arguments")
        }
    }

    /// Run with: `cargo test --release -p iota-adapter-latest --lib
    /// bench_native_call_hot_path -- --ignored --nocapture`
    #[test]
    #[ignore = "manual benchmark, run explicitly in release mode"]
    fn bench_native_call_hot_path() {
        let module_id = ModuleId::new(AccountAddress::TWO, Identifier::new("hash").unwrap());
        let arg = Value::vector_u8(vec![7u8; 10_000]);
        let mut status = GasStatus::new(initial_cost_schedule_v1(), u64::MAX / 2_000, 1, 1);
        let mut meter = IotaGasMeter(&mut status);
        let iterations: u64 = 100_000;
        let start = std::time::Instant::now();
        for _ in 0..iterations {
            meter.record_native_function_identity(&module_id, "blake2b256");
            meter
                .charge_native_function_before_execution(
                    std::iter::empty::<NoTypeArgs>(),
                    std::iter::once(&arg),
                )
                .unwrap();
            meter
                .charge_native_function(
                    InternalGas::new(100),
                    Option::<std::iter::Empty<&Value>>::None,
                )
                .unwrap();
        }
        let elapsed = start.elapsed();
        println!(
            "native call: {:.2} ns/call over {iterations} calls, one 10 KB vector argument \
             (total {elapsed:?})",
            elapsed.as_nanos() as f64 / iterations as f64,
        );
    }

    /// Run with: `cargo test --release -p iota-adapter-latest --lib
    /// bench_drop_frame_hot_path -- --ignored --nocapture`
    #[test]
    #[ignore = "manual benchmark, run explicitly in release mode"]
    fn bench_drop_frame_hot_path() {
        let local = Value::vector_u8(vec![7u8; 10_000]);
        let mut status = GasStatus::new(initial_cost_schedule_v1(), u64::MAX / 2_000, 1, 1);
        let mut meter = IotaGasMeter(&mut status);
        let iterations: u64 = 100_000;
        let start = std::time::Instant::now();
        for _ in 0..iterations {
            meter.0.record_call_frame(0);
            meter.charge_drop_frame(std::iter::once(&local)).unwrap();
        }
        let elapsed = start.elapsed();
        println!(
            "drop frame: {:.2} ns/call over {iterations} calls, one 10 KB vector local \
             (total {elapsed:?})",
            elapsed.as_nanos() as f64 / iterations as f64,
        );
    }

    /// Runs one native call through the meter and returns the per-function
    /// counter keys the profile recorded.
    fn native_call_keys(module: &str, function: &str, args: Vec<Value>) -> Vec<String> {
        let module_id = ModuleId::new(AccountAddress::TWO, Identifier::new(module).unwrap());
        let mut status = GasStatus::new(initial_cost_schedule_v1(), u64::MAX / 2_000, 1, 1);
        let mut meter = IotaGasMeter(&mut status);
        meter.record_native_function_identity(&module_id, function);
        meter
            .charge_native_function_before_execution(std::iter::empty::<NoTypeArgs>(), args.iter())
            .unwrap();
        meter
            .charge_native_function(
                InternalGas::new(100),
                Option::<std::iter::Empty<&Value>>::None,
            )
            .unwrap();
        status
            .resource_profile()
            .native_calls_by_function
            .keys()
            .cloned()
            .collect()
    }

    #[test]
    fn selector_refines_native_counter_key() {
        // groth16: curve selector is the first argument.
        let keys = native_call_keys(
            "groth16",
            "verify_groth16_proof_internal",
            vec![Value::u8(1), Value::vector_u8(vec![0u8; 32])],
        );
        assert_eq!(
            keys,
            vec!["0x2::groth16::verify_groth16_proof_internal::bn254".to_string()]
        );

        // group_ops: group selector is the first argument.
        let keys = native_call_keys(
            "group_ops",
            "internal_add",
            vec![
                Value::u8(2),
                Value::vector_u8(vec![0u8; 96]),
                Value::vector_u8(vec![0u8; 96]),
            ],
        );
        assert_eq!(
            keys,
            vec!["0x2::group_ops::internal_add::bls12381_g2".to_string()]
        );

        // ecdsa: hash selector is the last argument.
        let keys = native_call_keys(
            "ecdsa_k1",
            "secp256k1_verify",
            vec![
                Value::vector_u8(vec![0u8; 64]),
                Value::vector_u8(vec![0u8; 33]),
                Value::vector_u8(vec![0u8; 32]),
                Value::u8(0),
            ],
        );
        assert_eq!(
            keys,
            vec!["0x2::ecdsa_k1::secp256k1_verify::keccak256".to_string()]
        );

        // An out-of-range selector keeps the unrefined key.
        let keys = native_call_keys(
            "groth16",
            "verify_groth16_proof_internal",
            vec![Value::u8(9), Value::vector_u8(vec![0u8; 32])],
        );
        assert_eq!(
            keys,
            vec!["0x2::groth16::verify_groth16_proof_internal".to_string()]
        );

        // A native outside the registry keeps its plain key.
        let keys = native_call_keys("hash", "blake2b256", vec![Value::vector_u8(vec![0u8; 32])]);
        assert_eq!(keys, vec!["0x2::hash::blake2b256".to_string()]);
    }

    #[test]
    fn selector_rebuilds_key_between_calls() {
        let module_id = ModuleId::new(AccountAddress::TWO, Identifier::new("groth16").unwrap());
        let mut status = GasStatus::new(initial_cost_schedule_v1(), u64::MAX / 2_000, 1, 1);
        let mut meter = IotaGasMeter(&mut status);
        for curve in [1u8, 0u8] {
            let args = [Value::u8(curve)];
            meter.record_native_function_identity(&module_id, "verify_groth16_proof_internal");
            meter
                .charge_native_function_before_execution(
                    std::iter::empty::<NoTypeArgs>(),
                    args.iter(),
                )
                .unwrap();
            meter
                .charge_native_function(
                    InternalGas::new(100),
                    Option::<std::iter::Empty<&Value>>::None,
                )
                .unwrap();
        }
        let keys: Vec<String> = status
            .resource_profile()
            .native_calls_by_function
            .keys()
            .cloned()
            .collect();
        assert_eq!(
            keys,
            vec![
                "0x2::groth16::verify_groth16_proof_internal::bls12381".to_string(),
                "0x2::groth16::verify_groth16_proof_internal::bn254".to_string(),
            ]
        );
    }
}
