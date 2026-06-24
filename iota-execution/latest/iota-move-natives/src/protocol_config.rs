// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::VecDeque;

use iota_protocol_config::ProtocolConfigValue;
use move_binary_format::errors::PartialVMResult;
use move_vm_runtime::native_functions::NativeContext;
use move_vm_types::{
    loaded_data::runtime_types::Type,
    natives::function::NativeResult,
    pop_arg,
    values::{Value, Vector},
};
use smallvec::smallvec;

use crate::{get_extension, object_runtime::ObjectRuntime};

/// Abort code returned when the parameter name is not valid UTF-8.
const E_INVALID_UTF8_PARAM_NAME: u64 = 0;
/// Abort code returned when the parameter is absent in the current protocol
/// version.
const E_PARAM_NOT_FOUND: u64 = 1;
/// Abort code returned when the requested Move type does not match the actual
/// parameter type stored in the protocol config.
const E_TYPE_MISMATCH: u64 = 2;

/// ****************************************************************************
/// ********************* native fun is_feature_enabled
///
/// Implementation of the Move native function
/// `protocol_config::is_feature_enabled(feature_flag_name: vector<u8>): bool`
///
/// Checks if a protocol feature flag is enabled in the current protocol
/// version.
///
/// Gas cost: 0 (zero cost for framework-internal use)
/// ****************************************************************************
/// *******************
pub fn is_feature_enabled(
    context: &mut NativeContext,
    ty_args: Vec<Type>,
    mut args: VecDeque<Value>,
) -> PartialVMResult<NativeResult> {
    debug_assert!(ty_args.is_empty());
    debug_assert!(args.len() == 1);

    let feature_flag_name_bytes = pop_arg!(args, Vector);
    let bytes = feature_flag_name_bytes.to_vec_u8()?;

    let protocol_config = get_extension!(context, ObjectRuntime)?.protocol_config;

    let is_enabled = match String::from_utf8(bytes.to_vec()) {
        Ok(s) => {
            // Use the auto-generated lookup_feature method to find the feature flag
            match protocol_config.lookup_feature(s) {
                Some(value) => value,
                None => {
                    debug_assert!(false);
                    // We don't distinguish between feature flags that are not present and feature
                    // flags that are present but disabled. This is to handle
                    // the case where we accidentally shipped a framework upgrade that check a
                    // feature flag that is not present in the binary yet.
                    false
                }
            }
        }
        Err(_) => {
            debug_assert!(false);
            // Invalid UTF feature flags are treated as disabled feature flags.
            false
        }
    };

    Ok(NativeResult::ok(
        context.gas_used(),
        smallvec![Value::bool(is_enabled)],
    ))
}

/// ****************************************************************************
/// ********************* native fun get_attr
///
/// Implementation of the Move native function
/// `protocol_config::get_attr<T: copy + drop + store>(param_name: vector<u8>):
/// T`
///
/// Returns the parameter value directly.
///
/// Aborts with `E_INVALID_UTF8_PARAM_NAME` if `param_name` is not valid UTF-8,
/// with `E_PARAM_NOT_FOUND` if the parameter is absent in the current protocol
/// version, and with `E_TYPE_MISMATCH` if `T` does not match the parameter's
/// actual type — all three are programming errors that must not occur at
/// runtime.
///
/// Gas cost: 0 (zero cost for framework-internal use)
/// ****************************************************************************
/// *******************
pub fn get_attr(
    context: &mut NativeContext,
    ty_args: Vec<Type>,
    mut args: VecDeque<Value>,
) -> PartialVMResult<NativeResult> {
    debug_assert_eq!(ty_args.len(), 1);
    debug_assert_eq!(args.len(), 1);

    let ty = &ty_args[0];
    let param_name_bytes = pop_arg!(args, Vector).to_vec_u8()?;

    let param_name = match String::from_utf8(param_name_bytes) {
        Ok(name) => name,
        Err(_) => {
            return Ok(NativeResult::err(
                context.gas_used(),
                E_INVALID_UTF8_PARAM_NAME,
            ));
        }
    };

    let protocol_config = get_extension!(context, ObjectRuntime)?.protocol_config;

    let value = match (ty, protocol_config.lookup_attr(param_name)) {
        (Type::U64, Some(ProtocolConfigValue::u64(v))) => Value::u64(v),
        (Type::U32, Some(ProtocolConfigValue::u32(v))) => Value::u32(v),
        (Type::U16, Some(ProtocolConfigValue::u16(v))) => Value::u16(v),
        (Type::Bool, Some(ProtocolConfigValue::bool(v))) => Value::bool(v),

        // The parameter is absent in the current protocol version.
        (_, None) => {
            return Ok(NativeResult::err(context.gas_used(), E_PARAM_NOT_FOUND));
        }

        // The requested Move type does not match the actual parameter type.
        _ => {
            return Ok(NativeResult::err(context.gas_used(), E_TYPE_MISMATCH));
        }
    };

    Ok(NativeResult::ok(context.gas_used(), smallvec![value]))
}
