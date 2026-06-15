// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::VecDeque;

use iota_types::iota_system_state::attestor_registry::verify_attestor_pubkey;
use move_binary_format::errors::PartialVMResult;
use move_vm_runtime::{native_charge_gas_early_exit, native_functions::NativeContext};
use move_vm_types::{
    loaded_data::runtime_types::Type, natives::function::NativeResult, pop_arg, values::Value,
};
use smallvec::smallvec;

use crate::NativesCostTable;

/// ****************************************************************************
/// native fun validate_attestor_pubkey
/// Implementation of the Move native function
/// `validate_attestor_pubkey(pubkey: vector<u8>)`.
///
/// Delegates to `iota_types::iota_system_state::attestor_registry::
/// verify_attestor_pubkey`, which validates the `flag || raw_key` encoding
/// against the iota-rust-sdk public-key types (plain schemes only). Mirrors
/// `validator::validate_metadata_bcs` delegating to
/// `ValidatorMetadataV1::verify`.
///
/// gas cost: reuses the validator metadata validation cost params
///   validator_validate_metadata_cost_base
///     + validator_validate_metadata_data_cost_per_byte * pubkey.len()
/// ****************************************************************************
pub fn validate_attestor_pubkey(
    context: &mut NativeContext,
    ty_args: Vec<Type>,
    mut args: VecDeque<Value>,
) -> PartialVMResult<NativeResult> {
    debug_assert!(ty_args.is_empty());
    debug_assert!(args.len() == 1);

    let cost_params = context
        .extensions_mut()
        .get::<NativesCostTable>()?
        .validator_validate_metadata_bcs_cost_params
        .clone();

    native_charge_gas_early_exit!(context, cost_params.validator_validate_metadata_cost_base);

    let pubkey = pop_arg!(args, Vec<u8>);

    native_charge_gas_early_exit!(
        context,
        cost_params.validator_validate_metadata_data_cost_per_byte
            * (pubkey.len() as u64).into()
    );

    let cost = context.gas_used();

    if let Err(err_code) = verify_attestor_pubkey(&pubkey) {
        return Ok(NativeResult::err(cost, err_code));
    }

    Ok(NativeResult::ok(cost, smallvec![]))
}
