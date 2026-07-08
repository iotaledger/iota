// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::{BTreeMap, VecDeque};

use better_any::{Tid, TidAble};
use iota_protocol_config::ProtocolConfigValue;
use move_binary_format::errors::PartialVMResult;
use move_vm_runtime::{native_extensions::NativeExtensionMarker, native_functions::NativeContext};
use move_vm_types::{
    loaded_data::runtime_types::Type,
    natives::function::NativeResult,
    pop_arg,
    values::{Value, Vector},
};
use smallvec::smallvec;

use crate::{get_extension, get_extension_mut, object_runtime::ObjectRuntime};

/// Abort code returned when the parameter name is not valid UTF-8.
const E_INVALID_UTF8_PARAM_NAME: u64 = 0;
/// Abort code returned when the parameter is absent in the current protocol
/// version.
const E_PARAM_NOT_FOUND: u64 = 1;
/// Abort code returned when the requested Move type does not match the actual
/// parameter type stored in the protocol config.
const E_TYPE_MISMATCH: u64 = 2;

/// Per-test overrides for protocol config feature flags and parameters.
///
/// Added to the native context extensions only by the Move unit-test runner, so
/// it is absent during on-chain execution: [`is_feature_enabled`] and
/// [`get_attr`] then fall back to the protocol config unchanged.
#[derive(Tid, Default)]
pub struct ProtocolConfigTestOverrides {
    feature_flags: BTreeMap<String, bool>,
    attrs: BTreeMap<String, ProtocolConfigValue>,
}

impl NativeExtensionMarker<'_> for ProtocolConfigTestOverrides {}

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

    let is_enabled = match String::from_utf8(bytes.to_vec()) {
        Ok(s) => {
            // Per-test override set from Move test code. The extension is only added by
            // the unit-test runner, so on-chain execution skips this and takes the
            // protocol config path below unchanged.
            if let Ok(overrides) = context.extensions().get::<ProtocolConfigTestOverrides>() {
                if let Some(value) = overrides.feature_flags.get(&s).copied() {
                    return Ok(NativeResult::ok(
                        context.gas_used(),
                        smallvec![Value::bool(value)],
                    ));
                }
            }

            let protocol_config = get_extension!(context, ObjectRuntime)?.protocol_config;
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

    // Per-test override set from Move test code. The extension is only added by
    // the unit-test runner, so on-chain execution skips this and takes the
    // protocol config path below unchanged.
    let overridden = context
        .extensions()
        .get::<ProtocolConfigTestOverrides>()
        .ok()
        .and_then(|overrides| overrides.attrs.get(&param_name).cloned());
    let attr_value = match overridden {
        Some(value) => Some(value),
        None => protocol_config.lookup_attr(param_name),
    };

    let value = match (ty, attr_value) {
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

/// ****************************************************************************
/// ********************* native fun set_protocol_config_value_for_testing
///
/// Implementation of the test-only Move native function
/// `protocol_config::set_protocol_config_value_for_testing<T: copy + drop +
/// store>(name: vector<u8>, value: T)`
///
/// Sets a per-test override for a feature flag or a config parameter, read back
/// by [`is_feature_enabled`] and [`get_attr`]. `name` resolves to a feature
/// flag first (which must be a `bool`), otherwise to a config parameter (whose
/// type must match `T`). The override lives for the whole unit test and is
/// reset between tests, as the [`ProtocolConfigTestOverrides`] extension is
/// created fresh per test.
///
/// Aborts with `E_INVALID_UTF8_PARAM_NAME` if `name` is not valid UTF-8, with
/// `E_PARAM_NOT_FOUND` if it names neither a flag nor a parameter present in
/// the current protocol config, and with `E_TYPE_MISMATCH` if `T` does not
/// match the target's type — so a mistyped name or wrong type fails loudly.
/// ****************************************************************************
/// *******************
pub fn set_protocol_config_value_for_testing(
    context: &mut NativeContext,
    ty_args: Vec<Type>,
    mut args: VecDeque<Value>,
) -> PartialVMResult<NativeResult> {
    debug_assert_eq!(ty_args.len(), 1);
    debug_assert_eq!(args.len(), 2);

    let ty = &ty_args[0];
    // The value is the last argument, so it is popped first.
    let value = match ty {
        Type::U64 => ProtocolConfigValue::u64(pop_arg!(args, u64)),
        Type::U32 => ProtocolConfigValue::u32(pop_arg!(args, u32)),
        Type::U16 => ProtocolConfigValue::u16(pop_arg!(args, u16)),
        Type::Bool => ProtocolConfigValue::bool(pop_arg!(args, bool)),
        _ => return Ok(NativeResult::err(context.gas_used(), E_TYPE_MISMATCH)),
    };

    let name_bytes = pop_arg!(args, Vector).to_vec_u8()?;
    let name = match String::from_utf8(name_bytes) {
        Ok(name) => name,
        Err(_) => {
            return Ok(NativeResult::err(
                context.gas_used(),
                E_INVALID_UTF8_PARAM_NAME,
            ));
        }
    };

    let protocol_config = get_extension!(context, ObjectRuntime)?.protocol_config;

    // A feature flag takes precedence and must be a bool.
    if protocol_config.lookup_feature(name.clone()).is_some() {
        let ProtocolConfigValue::bool(flag_value) = value else {
            return Ok(NativeResult::err(context.gas_used(), E_TYPE_MISMATCH));
        };
        get_extension_mut!(context, ProtocolConfigTestOverrides)?
            .feature_flags
            .insert(name, flag_value);
        return Ok(NativeResult::ok(context.gas_used(), smallvec![]));
    }

    // Otherwise a config parameter: `T` must match its type in this version.
    match protocol_config.lookup_attr(name.clone()) {
        Some(existing) if std::mem::discriminant(&existing) == std::mem::discriminant(&value) => {
            get_extension_mut!(context, ProtocolConfigTestOverrides)?
                .attrs
                .insert(name, value);
            Ok(NativeResult::ok(context.gas_used(), smallvec![]))
        }
        Some(_) => Ok(NativeResult::err(context.gas_used(), E_TYPE_MISMATCH)),
        None => Ok(NativeResult::err(context.gas_used(), E_PARAM_NOT_FOUND)),
    }
}
