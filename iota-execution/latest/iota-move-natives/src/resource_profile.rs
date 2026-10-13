// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use move_core_types::{
    account_address::AccountAddress, gas_algebra::AbstractMemorySize, u256::U256,
};
use move_vm_types::views::{ValueView, ValueVisitor};

// Defined here rather than reused from `iota_types::gas_model::tables` so that
// each execution version cut gets its own copy.
const CONST_SIZE: AbstractMemorySize = AbstractMemorySize::new(16);
const REFERENCE_SIZE: AbstractMemorySize = AbstractMemorySize::new(8);
const STRUCT_SIZE: AbstractMemorySize = AbstractMemorySize::new(2);

/// Returns the abstract memory size of `value` for the resource profile,
/// following references only into primitive data (a scalar or a vector of
/// scalars). A reference to a struct, variant, or vector of containers counts
/// at its constant reference size, so the cost of computing the size stays
/// bounded. A value that holds no references is sized in full.
pub fn abstract_input_size(value: impl ValueView) -> AbstractMemorySize {
    let mut visitor = InputSizeVisitor {
        size: AbstractMemorySize::zero(),
        behind_ref: false,
    };
    value.visit(&mut visitor);
    visitor.size
}

struct InputSizeVisitor {
    size: AbstractMemorySize,
    behind_ref: bool,
}

impl InputSizeVisitor {
    fn visit_container(&mut self) -> bool {
        if self.behind_ref {
            return false;
        }
        self.size += STRUCT_SIZE;
        true
    }

    fn add_primitive_vec<T>(&mut self, vals: &[T]) {
        self.size += STRUCT_SIZE;
        self.size += AbstractMemorySize::new(std::mem::size_of_val(vals) as u64);
    }
}

impl ValueVisitor for InputSizeVisitor {
    fn visit_u8(&mut self, _depth: usize, _val: u8) {
        self.size += CONST_SIZE;
    }

    fn visit_u16(&mut self, _depth: usize, _val: u16) {
        self.size += CONST_SIZE;
    }

    fn visit_u32(&mut self, _depth: usize, _val: u32) {
        self.size += CONST_SIZE;
    }

    fn visit_u64(&mut self, _depth: usize, _val: u64) {
        self.size += CONST_SIZE;
    }

    fn visit_u128(&mut self, _depth: usize, _val: u128) {
        self.size += CONST_SIZE;
    }

    fn visit_u256(&mut self, _depth: usize, _val: U256) {
        self.size += CONST_SIZE;
    }

    fn visit_bool(&mut self, _depth: usize, _val: bool) {
        self.size += CONST_SIZE;
    }

    fn visit_address(&mut self, _depth: usize, _val: AccountAddress) {
        self.size += AbstractMemorySize::new(AccountAddress::LENGTH as u64);
    }

    fn visit_struct(&mut self, _depth: usize, _len: usize) -> bool {
        self.visit_container()
    }

    fn visit_variant(&mut self, _depth: usize, _len: usize) -> bool {
        self.visit_container()
    }

    fn visit_vec(&mut self, _depth: usize, _len: usize) -> bool {
        self.visit_container()
    }

    fn visit_ref(&mut self, _depth: usize, _is_global: bool) -> bool {
        self.size += REFERENCE_SIZE;
        self.behind_ref = true;
        true
    }

    fn visit_vec_u8(&mut self, _depth: usize, vals: &[u8]) {
        self.add_primitive_vec(vals);
    }

    fn visit_vec_u16(&mut self, _depth: usize, vals: &[u16]) {
        self.add_primitive_vec(vals);
    }

    fn visit_vec_u32(&mut self, _depth: usize, vals: &[u32]) {
        self.add_primitive_vec(vals);
    }

    fn visit_vec_u64(&mut self, _depth: usize, vals: &[u64]) {
        self.add_primitive_vec(vals);
    }

    fn visit_vec_u128(&mut self, _depth: usize, vals: &[u128]) {
        self.add_primitive_vec(vals);
    }

    fn visit_vec_u256(&mut self, _depth: usize, vals: &[U256]) {
        self.add_primitive_vec(vals);
    }

    fn visit_vec_bool(&mut self, _depth: usize, vals: &[bool]) {
        self.add_primitive_vec(vals);
    }

    fn visit_vec_address(&mut self, _depth: usize, vals: &[AccountAddress]) {
        self.add_primitive_vec(vals);
    }
}

#[cfg(test)]
mod tests {
    use move_vm_types::values::{Locals, Struct, Value, Variant, Vector, VectorSpecialization};

    use super::*;

    fn size(bytes: u64) -> AbstractMemorySize {
        AbstractMemorySize::new(bytes)
    }

    #[test]
    fn abstract_input_size_sizes_owned_values_in_full() {
        let cases = [
            (Value::u8(1), CONST_SIZE),
            (Value::u128(5), CONST_SIZE),
            (Value::u256(U256::max_value()), CONST_SIZE),
            (Value::bool(true), CONST_SIZE),
            (Value::address(AccountAddress::TWO), size(32)),
            (Value::vector_u8(vec![0u8; 33]), STRUCT_SIZE + size(33)),
            (Value::vector_u64(vec![7u64; 5]), STRUCT_SIZE + size(40)),
            (
                Value::vector_address(vec![AccountAddress::ONE; 3]),
                STRUCT_SIZE + size(96),
            ),
            (
                Value::struct_(Struct::pack([
                    Value::u64(1),
                    Value::vector_u8(vec![1, 2, 3]),
                    Value::struct_(Struct::pack([Value::bool(false)])),
                ])),
                STRUCT_SIZE + CONST_SIZE + STRUCT_SIZE + size(3) + STRUCT_SIZE + CONST_SIZE,
            ),
            (
                Value::variant(Variant::pack(1, [Value::u64(9), Value::u8(2)])),
                STRUCT_SIZE + CONST_SIZE + CONST_SIZE,
            ),
            (
                Vector::pack(
                    VectorSpecialization::Container,
                    (0..10).map(|i| Value::struct_(Struct::pack([Value::u64(i)]))),
                )
                .unwrap(),
                size(u64::from(STRUCT_SIZE) + 10 * u64::from(STRUCT_SIZE + CONST_SIZE)),
            ),
        ];
        for (value, expected) in &cases {
            assert_eq!(abstract_input_size(value), *expected);
        }
    }

    #[test]
    fn abstract_input_size_follows_refs_into_primitive_data_only() {
        let mut locals = Locals::new(4);

        locals
            .store_loc(0, Value::vector_u8(vec![0u8; 100]), true)
            .unwrap();
        assert_eq!(
            abstract_input_size(locals.borrow_loc(0).unwrap()),
            REFERENCE_SIZE + STRUCT_SIZE + size(100)
        );

        locals.store_loc(1, Value::u64(7), true).unwrap();
        assert_eq!(
            abstract_input_size(locals.borrow_loc(1).unwrap()),
            REFERENCE_SIZE + CONST_SIZE
        );

        locals
            .store_loc(
                2,
                Value::struct_(Struct::pack([Value::u64(1), Value::u64(2)])),
                true,
            )
            .unwrap();
        assert_eq!(
            abstract_input_size(locals.borrow_loc(2).unwrap()),
            REFERENCE_SIZE
        );

        let container_vec = Vector::pack(
            VectorSpecialization::Container,
            (0..1000).map(|i| Value::struct_(Struct::pack([Value::u64(i), Value::u64(i)]))),
        )
        .unwrap();
        locals.store_loc(3, container_vec, true).unwrap();
        assert_eq!(
            abstract_input_size(locals.borrow_loc(3).unwrap()),
            REFERENCE_SIZE
        );
    }
}
