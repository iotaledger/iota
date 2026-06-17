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
    values::{Struct, Value, Vector},
};
use smallvec::smallvec;

use crate::{get_extension, object_runtime::ObjectRuntime};

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

// Constructs a Move `Option<T>::none` value.
fn option_none(type_param: &Type) -> PartialVMResult<Value> {
    Ok(Value::struct_(Struct::pack(vec![Vector::empty(
        type_param,
    )?])))
}

// Constructs a Move `Option<T>::some(value)` value.
fn option_some(value: Value, type_param: &Type) -> PartialVMResult<Value> {
    Ok(Value::struct_(Struct::pack(vec![Vector::pack(
        type_param,
        vec![value],
    )?])))
}

/// ****************************************************************************
/// ********************* native fun get_attr
///
/// Implementation of the Move native function
/// `protocol_config::get_attr<T: copy + drop + store>(param_name: vector<u8>):
/// Option<T>`
///
/// Returns the value of a protocol config parameter, or `none` if the parameter
/// is not defined at the current protocol version or `T` does not match the
/// parameter's actual type.
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

    let output = match String::from_utf8(param_name_bytes) {
        Ok(param_name) => {
            let protocol_config = get_extension!(context, ObjectRuntime)?.protocol_config;

            match (ty, protocol_config.lookup_attr(param_name)) {
                (Type::U64, Some(ProtocolConfigValue::u64(value))) => option_some(Value::u64(value), ty)?,
                (Type::U32, Some(ProtocolConfigValue::u32(value))) => option_some(Value::u32(value), ty)?,
                (Type::U16, Some(ProtocolConfigValue::u16(value))) => option_some(Value::u16(value), ty)?,
                (Type::Bool, Some(ProtocolConfigValue::bool(value))) => option_some(Value::bool(value), ty)?,

                // The parameter is absent in the current protocol version.
                (_, None) => option_none(ty)?,

                // The requested Move type does not match the actual parameter type.
                _ => {
                    debug_assert!(
                        false,
                        "get_attr: type mismatch for protocol config parameter"
                    );
                    option_none(ty)?
                }
            }
        }

        // Invalid UTF-8 parameter names are treated as missing.
        Err(_) => option_none(ty)?,
    };

    Ok(NativeResult::ok(context.gas_used(), smallvec![output]))
}
