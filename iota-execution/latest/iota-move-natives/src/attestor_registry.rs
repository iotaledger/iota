// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::VecDeque;

use iota_sdk_types::Address;
use iota_types::iota_system_state::attestor_registry::{
    verify_attestor_pop, verify_attestor_pubkey,
};
use move_binary_format::errors::{PartialVMError, PartialVMResult};
use move_core_types::{
    account_address::AccountAddress, gas_algebra::InternalGas, vm_status::StatusCode,
};
use move_vm_runtime::{native_charge_gas_early_exit, native_functions::NativeContext};
use move_vm_types::{
    loaded_data::runtime_types::Type, natives::function::NativeResult, pop_arg, values::Value,
};
use smallvec::smallvec;

use crate::NativesCostTable;

#[derive(Clone)]
pub struct AttestorValidatePubkeyCostParams {
    /// Base cost for one key parse plus one signature verification; the
    /// inputs are fixed-size, so there is no per-byte term.
    pub attestor_validate_pubkey_cost_base: Option<InternalGas>,
}

/// ****************************************************************************
/// native fun validate_attestor_pubkey
/// Implementation of the Move native function
/// `validate_attestor_pubkey(pubkey: vector<u8>, proof_of_possession:
/// vector<u8>, sender: address)`.
///
/// Delegates to `iota_types::iota_system_state::attestor_registry::
/// verify_attestor_pubkey` and `verify_attestor_pop`, which validate the
/// `flag || raw_key` encoding and the raw-signature proof of possession
/// against the iota-rust-sdk public-key types (plain schemes only). Aborts
/// with `EInvalidPubkey` (3) or `EInvalidProofOfPossession` (8). Mirrors
/// `validator::validate_metadata_bcs` delegating to
/// `ValidatorMetadataV1::verify`.
///
/// gas cost: attestor_validate_pubkey_cost_base
/// ****************************************************************************
pub fn validate_attestor_pubkey(
    context: &mut NativeContext,
    ty_args: Vec<Type>,
    mut args: VecDeque<Value>,
) -> PartialVMResult<NativeResult> {
    debug_assert!(ty_args.is_empty());
    debug_assert!(args.len() == 3);

    let cost_params = context
        .extensions_mut()
        .get::<NativesCostTable>()?
        .attestor_validate_pubkey_cost_params
        .clone();

    native_charge_gas_early_exit!(
        context,
        cost_params
            .attestor_validate_pubkey_cost_base
            .ok_or_else(|| {
                PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR).with_message(
                    "Gas cost base for validate_attestor_pubkey not available".to_string(),
                )
            })?
    );

    // Args are popped in reverse declaration order.
    let sender = pop_arg!(args, AccountAddress);
    let pop = pop_arg!(args, Vec<u8>);
    let pubkey = pop_arg!(args, Vec<u8>);

    let cost = context.gas_used();

    if let Err(err_code) = verify_attestor_pubkey(&pubkey) {
        return Ok(NativeResult::err(cost, err_code));
    }
    let sender = Address::new(sender.into_bytes());
    if let Err(err_code) = verify_attestor_pop(&pubkey, &pop, sender) {
        return Ok(NativeResult::err(cost, err_code));
    }

    Ok(NativeResult::ok(cost, smallvec![]))
}
