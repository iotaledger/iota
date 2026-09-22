// Copyright (c) The Move Contributors
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use move_core_types::{
    account_address::AccountAddress, gas_algebra::AbstractMemorySize, language_storage::TypeTag,
};

/// Trait that provides an abstract view into a Move type.
///
/// This is used to expose certain info to clients (e.g. the gas meter),
/// usually in a lazily evaluated fashion.
pub trait TypeView {
    /// Returns the `TypeTag` (fully qualified name) of the type.
    fn to_type_tag(&self) -> TypeTag;
}

/// Trait that provides an abstract view into a Move Value.
///
/// This is used to expose certain info to clients (e.g. the gas meter),
/// usually in a lazily evaluated fashion.
pub trait ValueView {
    fn visit(&self, visitor: &mut impl ValueVisitor);

    /// Returns the abstract memory size of the value.
    ///
    /// This version of abstract memory size is not well-defined and is only
    /// kept for backward compatibility.  New applications should avoid
    /// using this.
    fn legacy_abstract_memory_size(&self) -> AbstractMemorySize {
        use crate::values::{LEGACY_CONST_SIZE, LEGACY_REFERENCE_SIZE, LEGACY_STRUCT_SIZE};

        struct Acc(AbstractMemorySize);

        impl ValueVisitor for Acc {
            fn visit_u8(&mut self, _depth: usize, _val: u8) {
                self.0 += LEGACY_CONST_SIZE;
            }

            fn visit_u16(&mut self, _depth: usize, _val: u16) {
                self.0 += LEGACY_CONST_SIZE;
            }

            fn visit_u32(&mut self, _depth: usize, _val: u32) {
                self.0 += LEGACY_CONST_SIZE;
            }

            fn visit_u64(&mut self, _depth: usize, _val: u64) {
                self.0 += LEGACY_CONST_SIZE;
            }

            fn visit_u128(&mut self, _depth: usize, _val: u128) {
                self.0 += LEGACY_CONST_SIZE;
            }

            fn visit_u256(&mut self, _depth: usize, _val: move_core_types::u256::U256) {
                self.0 += LEGACY_CONST_SIZE;
            }

            fn visit_bool(&mut self, _depth: usize, _val: bool) {
                self.0 += LEGACY_CONST_SIZE;
            }

            fn visit_address(&mut self, _depth: usize, _val: AccountAddress) {
                self.0 += AbstractMemorySize::new(AccountAddress::LENGTH as u64);
            }

            fn visit_struct(&mut self, _depth: usize, _len: usize) -> bool {
                self.0 += LEGACY_STRUCT_SIZE;
                true
            }

            fn visit_variant(&mut self, _depth: usize, _len: usize) -> bool {
                self.0 += LEGACY_STRUCT_SIZE;
                true
            }

            fn visit_vec(&mut self, _depth: usize, _len: usize) -> bool {
                self.0 += LEGACY_STRUCT_SIZE;
                true
            }

            fn visit_vec_u8(&mut self, _depth: usize, vals: &[u8]) {
                self.0 += (std::mem::size_of_val(vals) as u64).into();
            }

            fn visit_vec_u16(&mut self, _depth: usize, vals: &[u16]) {
                self.0 += (std::mem::size_of_val(vals) as u64).into();
            }

            fn visit_vec_u32(&mut self, _depth: usize, vals: &[u32]) {
                self.0 += (std::mem::size_of_val(vals) as u64).into();
            }

            fn visit_vec_u64(&mut self, _depth: usize, vals: &[u64]) {
                self.0 += (std::mem::size_of_val(vals) as u64).into();
            }

            fn visit_vec_u128(&mut self, _depth: usize, vals: &[u128]) {
                self.0 += (std::mem::size_of_val(vals) as u64).into();
            }

            fn visit_vec_u256(&mut self, _depth: usize, vals: &[move_core_types::u256::U256]) {
                self.0 += (std::mem::size_of_val(vals) as u64).into();
            }

            fn visit_vec_bool(&mut self, _depth: usize, vals: &[bool]) {
                self.0 += (std::mem::size_of_val(vals) as u64).into();
            }

            fn visit_vec_address(&mut self, _depth: usize, vals: &[AccountAddress]) {
                self.0 += (std::mem::size_of_val(vals) as u64).into();
            }

            fn visit_ref(&mut self, _depth: usize, _is_global: bool) -> bool {
                self.0 += LEGACY_REFERENCE_SIZE;
                false
            }
        }

        let mut acc = Acc(0.into());
        self.visit(&mut acc);

        acc.0
    }

    /// Returns the abstract memory size of the value.
    fn abstract_memory_size(&self, traverse: bool) -> AbstractMemorySize {
        use crate::values::{LEGACY_CONST_SIZE, LEGACY_REFERENCE_SIZE, LEGACY_STRUCT_SIZE};

        struct Acc(AbstractMemorySize, bool);

        impl ValueVisitor for Acc {
            fn visit_u8(&mut self, _depth: usize, _val: u8) {
                self.0 += LEGACY_CONST_SIZE;
            }

            fn visit_u16(&mut self, _depth: usize, _val: u16) {
                self.0 += LEGACY_CONST_SIZE;
            }

            fn visit_u32(&mut self, _depth: usize, _val: u32) {
                self.0 += LEGACY_CONST_SIZE;
            }

            fn visit_u64(&mut self, _depth: usize, _val: u64) {
                self.0 += LEGACY_CONST_SIZE;
            }

            fn visit_u128(&mut self, _depth: usize, _val: u128) {
                self.0 += LEGACY_CONST_SIZE;
            }

            fn visit_u256(&mut self, _depth: usize, _val: move_core_types::u256::U256) {
                self.0 += LEGACY_CONST_SIZE;
            }

            fn visit_bool(&mut self, _depth: usize, _val: bool) {
                self.0 += LEGACY_CONST_SIZE;
            }

            fn visit_address(&mut self, _depth: usize, _val: AccountAddress) {
                self.0 += AbstractMemorySize::new(AccountAddress::LENGTH as u64);
            }

            fn visit_struct(&mut self, _depth: usize, _len: usize) -> bool {
                self.0 += LEGACY_STRUCT_SIZE;
                true
            }

            fn visit_variant(&mut self, _depth: usize, _len: usize) -> bool {
                self.0 += LEGACY_STRUCT_SIZE;
                true
            }

            fn visit_vec(&mut self, _depth: usize, _len: usize) -> bool {
                self.0 += LEGACY_STRUCT_SIZE;
                true
            }

            fn visit_vec_u8(&mut self, _depth: usize, vals: &[u8]) {
                self.0 += LEGACY_STRUCT_SIZE;
                self.0 += (std::mem::size_of_val(vals) as u64).into();
            }

            fn visit_vec_u16(&mut self, _depth: usize, vals: &[u16]) {
                self.0 += LEGACY_STRUCT_SIZE;
                self.0 += (std::mem::size_of_val(vals) as u64).into();
            }

            fn visit_vec_u32(&mut self, _depth: usize, vals: &[u32]) {
                self.0 += LEGACY_STRUCT_SIZE;
                self.0 += (std::mem::size_of_val(vals) as u64).into();
            }

            fn visit_vec_u64(&mut self, _depth: usize, vals: &[u64]) {
                self.0 += LEGACY_STRUCT_SIZE;
                self.0 += (std::mem::size_of_val(vals) as u64).into();
            }

            fn visit_vec_u128(&mut self, _depth: usize, vals: &[u128]) {
                self.0 += LEGACY_STRUCT_SIZE;
                self.0 += (std::mem::size_of_val(vals) as u64).into();
            }

            fn visit_vec_u256(&mut self, _depth: usize, vals: &[move_core_types::u256::U256]) {
                self.0 += LEGACY_STRUCT_SIZE;
                self.0 += (std::mem::size_of_val(vals) as u64).into();
            }

            fn visit_vec_bool(&mut self, _depth: usize, vals: &[bool]) {
                self.0 += LEGACY_STRUCT_SIZE;
                self.0 += (std::mem::size_of_val(vals) as u64).into();
            }

            fn visit_vec_address(&mut self, _depth: usize, vals: &[AccountAddress]) {
                self.0 += LEGACY_STRUCT_SIZE;
                self.0 += (std::mem::size_of_val(vals) as u64).into();
            }

            fn visit_ref(&mut self, _depth: usize, _is_global: bool) -> bool {
                self.0 += LEGACY_REFERENCE_SIZE;
                self.1
            }
        }

        let mut acc = Acc(0.into(), traverse);
        self.visit(&mut acc);

        acc.0
    }

    /// Returns the abstract memory size of the value as consumed by a
    /// byte-reading native function: references are followed only when they
    /// point at primitive data (a scalar, or a vector of scalars, whose size
    /// is known without walking its elements); a reference to a struct,
    /// variant, or vector of containers counts at its constant reference
    /// size, like in [`abstract_memory_size`](Self::abstract_memory_size)
    /// without traversal.
    ///
    /// This bounds the cost of computing the size itself: sizing structured
    /// data through a reference would visit every value in it on every call,
    /// work that no gas charge covers. The natives whose cost scales with
    /// their input read primitive byte vectors, so their inputs are still
    /// sized in full.
    fn abstract_input_size(&self) -> AbstractMemorySize {
        self.abstract_memory_and_input_size().1
    }

    /// Returns the value's
    /// [`abstract_memory_size`](Self::abstract_memory_size) without traversal
    /// and its [`abstract_input_size`](Self::abstract_input_size) as a pair,
    /// accumulated in a single visit of the value instead of one visit per
    /// size. The two sums differ only for data behind a reference: the first
    /// counts the reference at its constant size, the second follows it into
    /// primitive data.
    fn abstract_memory_and_input_size(&self) -> (AbstractMemorySize, AbstractMemorySize) {
        use crate::values::{LEGACY_CONST_SIZE, LEGACY_REFERENCE_SIZE, LEGACY_STRUCT_SIZE};

        struct Acc {
            memory: AbstractMemorySize,
            input: AbstractMemorySize,
            behind_ref: bool,
        }

        impl Acc {
            fn add(&mut self, size: AbstractMemorySize) {
                if !self.behind_ref {
                    self.memory += size;
                }
                self.input += size;
            }
        }

        impl ValueVisitor for Acc {
            fn visit_u8(&mut self, _depth: usize, _val: u8) {
                self.add(LEGACY_CONST_SIZE);
            }

            fn visit_u16(&mut self, _depth: usize, _val: u16) {
                self.add(LEGACY_CONST_SIZE);
            }

            fn visit_u32(&mut self, _depth: usize, _val: u32) {
                self.add(LEGACY_CONST_SIZE);
            }

            fn visit_u64(&mut self, _depth: usize, _val: u64) {
                self.add(LEGACY_CONST_SIZE);
            }

            fn visit_u128(&mut self, _depth: usize, _val: u128) {
                self.add(LEGACY_CONST_SIZE);
            }

            fn visit_u256(&mut self, _depth: usize, _val: move_core_types::u256::U256) {
                self.add(LEGACY_CONST_SIZE);
            }

            fn visit_bool(&mut self, _depth: usize, _val: bool) {
                self.add(LEGACY_CONST_SIZE);
            }

            fn visit_address(&mut self, _depth: usize, _val: AccountAddress) {
                self.add(AbstractMemorySize::new(AccountAddress::LENGTH as u64));
            }

            fn visit_struct(&mut self, _depth: usize, _len: usize) -> bool {
                if self.behind_ref {
                    return false;
                }
                self.add(LEGACY_STRUCT_SIZE);
                true
            }

            fn visit_variant(&mut self, _depth: usize, _len: usize) -> bool {
                if self.behind_ref {
                    return false;
                }
                self.add(LEGACY_STRUCT_SIZE);
                true
            }

            fn visit_vec(&mut self, _depth: usize, _len: usize) -> bool {
                if self.behind_ref {
                    return false;
                }
                self.add(LEGACY_STRUCT_SIZE);
                true
            }

            fn visit_vec_u8(&mut self, _depth: usize, vals: &[u8]) {
                self.add(LEGACY_STRUCT_SIZE);
                self.add((std::mem::size_of_val(vals) as u64).into());
            }

            fn visit_vec_u16(&mut self, _depth: usize, vals: &[u16]) {
                self.add(LEGACY_STRUCT_SIZE);
                self.add((std::mem::size_of_val(vals) as u64).into());
            }

            fn visit_vec_u32(&mut self, _depth: usize, vals: &[u32]) {
                self.add(LEGACY_STRUCT_SIZE);
                self.add((std::mem::size_of_val(vals) as u64).into());
            }

            fn visit_vec_u64(&mut self, _depth: usize, vals: &[u64]) {
                self.add(LEGACY_STRUCT_SIZE);
                self.add((std::mem::size_of_val(vals) as u64).into());
            }

            fn visit_vec_u128(&mut self, _depth: usize, vals: &[u128]) {
                self.add(LEGACY_STRUCT_SIZE);
                self.add((std::mem::size_of_val(vals) as u64).into());
            }

            fn visit_vec_u256(&mut self, _depth: usize, vals: &[move_core_types::u256::U256]) {
                self.add(LEGACY_STRUCT_SIZE);
                self.add((std::mem::size_of_val(vals) as u64).into());
            }

            fn visit_vec_bool(&mut self, _depth: usize, vals: &[bool]) {
                self.add(LEGACY_STRUCT_SIZE);
                self.add((std::mem::size_of_val(vals) as u64).into());
            }

            fn visit_vec_address(&mut self, _depth: usize, vals: &[AccountAddress]) {
                self.add(LEGACY_STRUCT_SIZE);
                self.add((std::mem::size_of_val(vals) as u64).into());
            }

            fn visit_ref(&mut self, _depth: usize, _is_global: bool) -> bool {
                // The reference itself is part of both sums; only what lies
                // behind it is treated differently.
                self.memory += LEGACY_REFERENCE_SIZE;
                self.input += LEGACY_REFERENCE_SIZE;
                self.behind_ref = true;
                true
            }
        }

        let mut acc = Acc {
            memory: 0.into(),
            input: 0.into(),
            behind_ref: false,
        };
        self.visit(&mut acc);

        (acc.memory, acc.input)
    }
}

/// Trait that defines a visitor that could be used to traverse a value
/// recursively.
pub trait ValueVisitor {
    fn visit_u8(&mut self, depth: usize, val: u8);
    fn visit_u16(&mut self, depth: usize, val: u16);
    fn visit_u32(&mut self, depth: usize, val: u32);
    fn visit_u64(&mut self, depth: usize, val: u64);
    fn visit_u128(&mut self, depth: usize, val: u128);
    fn visit_u256(&mut self, depth: usize, val: move_core_types::u256::U256);
    fn visit_bool(&mut self, depth: usize, val: bool);
    fn visit_address(&mut self, depth: usize, val: AccountAddress);

    fn visit_struct(&mut self, depth: usize, len: usize) -> bool;
    fn visit_variant(&mut self, depth: usize, len: usize) -> bool;
    fn visit_vec(&mut self, depth: usize, len: usize) -> bool;

    fn visit_ref(&mut self, depth: usize, is_global: bool) -> bool;

    fn visit_vec_u8(&mut self, depth: usize, vals: &[u8]) {
        self.visit_vec(depth, vals.len());
        for val in vals {
            self.visit_u8(depth + 1, *val);
        }
    }

    fn visit_vec_u16(&mut self, depth: usize, vals: &[u16]) {
        self.visit_vec(depth, vals.len());
        for val in vals {
            self.visit_u16(depth + 1, *val);
        }
    }

    fn visit_vec_u32(&mut self, depth: usize, vals: &[u32]) {
        self.visit_vec(depth, vals.len());
        for val in vals {
            self.visit_u32(depth + 1, *val);
        }
    }

    fn visit_vec_u64(&mut self, depth: usize, vals: &[u64]) {
        self.visit_vec(depth, vals.len());
        for val in vals {
            self.visit_u64(depth + 1, *val);
        }
    }

    fn visit_vec_u128(&mut self, depth: usize, vals: &[u128]) {
        self.visit_vec(depth, vals.len());
        for val in vals {
            self.visit_u128(depth + 1, *val);
        }
    }

    fn visit_vec_u256(&mut self, depth: usize, vals: &[move_core_types::u256::U256]) {
        self.visit_vec(depth, vals.len());
        for val in vals {
            self.visit_u256(depth + 1, *val);
        }
    }

    fn visit_vec_bool(&mut self, depth: usize, vals: &[bool]) {
        self.visit_vec(depth, vals.len());
        for val in vals {
            self.visit_bool(depth + 1, *val);
        }
    }

    fn visit_vec_address(&mut self, depth: usize, vals: &[AccountAddress]) {
        self.visit_vec(depth, vals.len());
        for val in vals {
            self.visit_address(depth + 1, *val);
        }
    }
}

impl<T> ValueView for &T
where
    T: ValueView,
{
    fn legacy_abstract_memory_size(&self) -> AbstractMemorySize {
        <T as ValueView>::legacy_abstract_memory_size(*self)
    }

    fn visit(&self, visitor: &mut impl ValueVisitor) {
        <T as ValueView>::visit(*self, visitor)
    }
}

impl<T> TypeView for &T
where
    T: TypeView,
{
    fn to_type_tag(&self) -> TypeTag {
        <T as TypeView>::to_type_tag(*self)
    }
}
