// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::VecDeque;

use iota_types::iota_system_state::iota_system_state_inner_v1::ValidatorMetadataV1;
use move_binary_format::errors::{PartialVMError, PartialVMResult};
use move_core_types::{gas_algebra::InternalGas, vm_status::StatusCode};
use move_vm_runtime::{
    execution::{Type, values::Value},
    native_charge_gas_early_exit,
    natives::functions::{NativeContext, NativeResult},
    pop_arg,
};
use smallvec::smallvec;

use crate::{NativesCostTable, get_extension};

#[derive(Clone, Debug)]
pub struct ValidatorValidateMetadataBcsCostParams {
    pub validator_validate_metadata_cost_base: InternalGas,
    pub validator_validate_metadata_data_cost_per_byte: InternalGas,
}
/// ****************************************************************************
/// ********************* native fun validate_metadata_bcs
/// Implementation of the Move native function `validate_metadata_bcs(metadata:
/// vector<u8>)`   gas cost: validator_validate_metadata_cost_base           |
/// fixed cosrs
///              + validator_validate_metadata_data_cost_per_byte * metadata_bytes.len()   | assume
///                cost is proportional to size
/// ****************************************************************************
/// *******************
pub fn validate_metadata_bcs(
    context: &mut NativeContext,
    ty_args: Vec<Type>,
    mut args: VecDeque<Value>,
) -> PartialVMResult<NativeResult> {
    debug_assert!(ty_args.is_empty());
    debug_assert!(args.len() == 1);

    let validator_validate_metadata_bcs_cost_params = get_extension!(context, NativesCostTable)?
        .validator_validate_metadata_bcs_cost_params
        .clone();

    native_charge_gas_early_exit!(
        context,
        validator_validate_metadata_bcs_cost_params.validator_validate_metadata_cost_base
    );

    let metadata_bytes = pop_arg!(args, Vec<u8>);

    native_charge_gas_early_exit!(
        context,
        validator_validate_metadata_bcs_cost_params
            .validator_validate_metadata_data_cost_per_byte
            .saturating_mul(metadata_bytes.len() as u64)
    );

    let validator_metadata =
        bcs::from_bytes::<ValidatorMetadataV1>(&metadata_bytes).map_err(|_| {
            PartialVMError::new(StatusCode::MALFORMED).with_message(
                "ValidatorMetadata Move struct does not match internal ValidatorMetadataV1 struct"
                    .to_string(),
            )
        })?;

    let cost = context.gas_used();

    if let Result::Err(err_code) = validator_metadata.verify(true) {
        return Ok(NativeResult::err(cost, err_code));
    }

    Ok(NativeResult::ok(cost, smallvec![]))
}
