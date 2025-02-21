// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::VecDeque;

use move_binary_format::errors::PartialVMResult;
use move_vm_runtime::native_functions::NativeContext;
use move_vm_types::{
    loaded_data::runtime_types::Type, natives::function::NativeResult, values::Value,
};
use rand::Rng;
use smallvec::smallvec;

use crate::legacy_test_cost;

pub fn generate_rand_seed_for_testing(
    _context: &mut NativeContext,
    _ty_args: Vec<Type>,
    _args: VecDeque<Value>,
) -> PartialVMResult<NativeResult> {
    let mut seed = [0u8; 32];
    rand::thread_rng()
        .try_fill(&mut seed)
        .expect("should never fail");
    Ok(NativeResult::ok(
        legacy_test_cost(),
        smallvec![Value::vector_u8(seed)],
    ))
}
