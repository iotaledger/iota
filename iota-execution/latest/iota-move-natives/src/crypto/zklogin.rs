// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Deprecated stubs for removed zklogin native functions.
// These must remain registered so that older bytecode snapshots (which still
// declare these natives) can be verified/linked by the VM during genesis.
// The Move-side wrappers already `assert!(false)`, so these are unreachable
// at runtime.

use std::collections::VecDeque;

use move_binary_format::errors::PartialVMResult;
use move_vm_runtime::native_functions::NativeContext;
use move_vm_types::{
    loaded_data::runtime_types::Type, natives::function::NativeResult, values::Value,
};

// The 20001 was chosen so it doesn't conflict with any existing error codes in
// the Move VM (which are all < 10000) and is clearly identifiable as a
// custom error code.
pub const NOT_SUPPORTED_ERROR: u64 = 20001;

#[deprecated(
    note = "zklogin natives have been removed; kept only for old bytecode snapshot compatibility"
)]
pub fn check_zklogin_id_internal(
    context: &mut NativeContext,
    _ty_args: Vec<Type>,
    _args: VecDeque<Value>,
) -> PartialVMResult<NativeResult> {
    Ok(NativeResult::err(context.gas_used(), NOT_SUPPORTED_ERROR))
}

#[deprecated(
    note = "zklogin natives have been removed; kept only for old bytecode snapshot compatibility"
)]
pub fn check_zklogin_issuer_internal(
    context: &mut NativeContext,
    _ty_args: Vec<Type>,
    _args: VecDeque<Value>,
) -> PartialVMResult<NativeResult> {
    Ok(NativeResult::err(context.gas_used(), NOT_SUPPORTED_ERROR))
}
