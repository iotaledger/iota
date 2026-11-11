// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::VecDeque;

use move_binary_format::errors::{PartialVMError, PartialVMResult};
use move_core_types::{gas_algebra::InternalGas, vm_status::StatusCode};
use move_vm_runtime::{native_charge_gas_early_exit, native_functions::NativeContext};
use move_vm_types::{
    loaded_data::runtime_types::Type, natives::function::NativeResult, pop_arg, values::Value,
};
use smallvec::smallvec;

use crate::{NativesCostTable, transaction_context::TransactionContext};

#[derive(Clone)]
pub struct VerifiedNonceTakeLeaseNonceKeyCostParams {
    pub verified_nonce_take_lease_nonce_key_cost_base: Option<InternalGas>,
}

/// Implementation of the Move native function
/// `verified_nonce::take_lease_nonce_key(nonce_key: u64): bool`
///
/// Records that `nonce_key` was taken under the lease of the transaction being
/// executed. Returns false if `nonce_key` was already taken in this
/// transaction.
///
/// gas cost: verified_nonce_take_lease_nonce_key_cost_base
pub fn take_lease_nonce_key(
    context: &mut NativeContext,
    ty_args: Vec<Type>,
    mut args: VecDeque<Value>,
) -> PartialVMResult<NativeResult> {
    debug_assert!(ty_args.is_empty());
    debug_assert!(args.len() == 1);

    let cost_base = context
        .extensions()
        .get::<NativesCostTable>()?
        .verified_nonce_take_lease_nonce_key_cost_params
        .verified_nonce_take_lease_nonce_key_cost_base
        .ok_or_else(|| {
            PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR)
                .with_message("Gas cost for take_lease_nonce_key not available".to_string())
        })?;
    native_charge_gas_early_exit!(context, cost_base);

    let nonce_key = pop_arg!(args, u64);

    let transaction_context: &mut TransactionContext = context.extensions_mut().get_mut()?;
    let taken = transaction_context.take_lease_nonce_key(nonce_key);

    Ok(NativeResult::ok(
        context.gas_used(),
        smallvec![Value::bool(taken)],
    ))
}
