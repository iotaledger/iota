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
use move_core_types::gas_algebra::InternalGas;
use move_vm_runtime::native_functions::NativeContext;
use move_vm_types::{
    loaded_data::runtime_types::Type, natives::function::NativeResult, values::Value,
};

#[deprecated(
    note = "zklogin natives have been removed; kept only for old bytecode snapshot compatibility"
)]
#[derive(Clone)]
pub struct CheckZkloginIdCostParams {
    pub check_zklogin_id_cost_base: Option<InternalGas>,
}

#[deprecated(
    note = "zklogin natives have been removed; kept only for old bytecode snapshot compatibility"
)]
#[derive(Clone)]
pub struct CheckZkloginIssuerCostParams {
    pub check_zklogin_issuer_cost_base: Option<InternalGas>,
}

#[deprecated(
    note = "zklogin natives have been removed; kept only for old bytecode snapshot compatibility"
)]
pub fn check_zklogin_id_internal(
    context: &mut NativeContext,
    _ty_args: Vec<Type>,
    _args: VecDeque<Value>,
) -> PartialVMResult<NativeResult> {
    Ok(NativeResult::err(context.gas_used(), 0))
}

#[deprecated(
    note = "zklogin natives have been removed; kept only for old bytecode snapshot compatibility"
)]
pub fn check_zklogin_issuer_internal(
    context: &mut NativeContext,
    _ty_args: Vec<Type>,
    _args: VecDeque<Value>,
) -> PartialVMResult<NativeResult> {
    Ok(NativeResult::err(context.gas_used(), 0))
}
