// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! A module can define a list of authenticate functions that can be used
//! to authenticate an account. This list is private to the module and
//! cannot be modified by other modules.
//! The list is defined as a constant vector of vector<u8> where each inner
//! vector<u8> is the UTF-8 bytes of the function name.
//!
//! The authenticate functions must be defined in the same module as the
//! constant.
//! The module's `init` function must call
//! `iota::account::publish_authenticate_registry` exactly once, passing the
//! constant as the argument. The `publish_authenticate_registry` function is
//! responsible for registering the authenticate functions and is defined in the
//! `iota::account`.
use iota_types::{IOTA_FRAMEWORK_ADDRESS, Identifier, error::ExecutionError};
use move_binary_format::{
    file_format::{
        Bytecode, CompiledModule, ConstantPoolIndex, FunctionInstantiation, SignatureToken,
    },
    internals::ModuleIndex,
};
use move_core_types::{ident_str, identifier::IdentStr, runtime_value::MoveValue};

use crate::{INIT_FN_NAME, account_auth_verifier::verify_authenticate_func, verification_failure};

pub const ACCOUNT_MODULE: &IdentStr = ident_str!("account");
pub const PUBLISH_AUTHENTICATE_REGISTRY_FN_NAME: &IdentStr =
    ident_str!("publish_authenticate_registry");

/// Checks if the module conforms to the authenticate functions rules only if it
/// has a call instruction to the `0x2::account::publish_authenticate_registry`
/// function within the `init` function.
///
/// If the module does not have such call instruction, then it is considered to
/// not use authenticate functions and thus the module is considered valid.
/// If the module does have such call instruction, then it must conform to the
/// rules.
pub fn verify_module(view: &CompiledModule) -> Result<(), ExecutionError> {
    // For each function in the module, look for the `init` function
    for fdef in &view.function_defs {
        let candidate_init_fn_name =
            view.identifier_at(view.function_handle_at(fdef.function).name);
        if candidate_init_fn_name == INIT_FN_NAME {
            // If init has some code
            let init_code = &match &fdef.code {
                None => return Ok(()),
                Some(code) => code,
            }
            .code;
            // verify that, if it calls `0x2::account::publish_authenticate_registry` then
            // it does with the authenticate functions constant as argument
            verify_init_publish_authenticate_registry(view, init_code)
                .map_err(verification_failure)?;
        }
    }
    Ok(())
}

/// Look for a constant that can represent a list of authenticate functions. The
/// constant must be of type `vector<vector<u8>>` and each inner vector<u8> must
/// be a valid UTF-8 string that can be serialized into the name of a function
/// in the module. Such function must also conform to the rules for authenticate
/// functions.
pub fn verify_authenticate_functions_const(
    view: &CompiledModule,
    index: ConstantPoolIndex,
) -> Result<(), String> {
    let candidate_const = view.constant_at(index);

    // Look for the type vector<vector<u8>>
    if let SignatureToken::Vector(const_vec_inner) = &candidate_const.type_ {
        if let SignatureToken::Vector(const_vec_inner_vec_inner) = const_vec_inner.as_ref() {
            if let SignatureToken::U8 = const_vec_inner_vec_inner.as_ref() {
                // Having found that, extract the inner list of vector<u8>
                if let MoveValue::Vector(candidate_const_vec_value) = candidate_const
                    .deserialize_constant()
                    .ok_or_else(|| format!("Malformed constant data"))?
                {
                    for candidate_const_vec_inner in candidate_const_vec_value {
                        // For each inner vector<u8>, check if it is a valid UTF-8 string
                        if let MoveValue::Vector(inner_bytes) = candidate_const_vec_inner {
                            let candidate_func_name_bytes = inner_bytes
                                .into_iter()
                                .map(|b| {
                                    if let MoveValue::U8(byte) = b {
                                        Ok(byte)
                                    } else {
                                        Err(format!("Unexpected value in bytes: {:?}", b))
                                    }
                                })
                                .collect::<Result<Vec<u8>, String>>()?;

                            // If it is, check if there is a function with that name in the
                            // module and if it conforms to the rules for authenticate
                            // functions.
                            if let Ok(candidate_func_name) =
                                String::from_utf8(candidate_func_name_bytes)
                            {
                                if let Ok(candidate_func_name_ident) =
                                    Identifier::new(candidate_func_name.as_str())
                                {
                                    verify_authenticate_func(
                                        view,
                                        candidate_func_name_ident.clone(),
                                    ).map_err( |e| format!("authenticate function '{}' does not conform to the rules: {}", candidate_func_name_ident, e) )?
                                } else {
                                    return Err(format!(
                                        "Expected the function name to be a valid identifier: {:?}",
                                        candidate_func_name
                                    ));
                                }
                            } else {
                                return Err(format!("Expected the function name bytes to be UTF8"));
                            }
                        }
                    }
                    // If it exited the loop, then all inner vector<u8> are valid
                    // UTF-8 strings and correspond to functions in the module that
                    // conform to the rules for authenticate functions.
                    return Ok(());
                };
            }
        }
    }

    Err(format!(
        "Expected a constant of type vector<vector<u8>> at index {}",
        index.into_index()
    ))
}

/// Check that the module's `init` function calls
/// `0x2::account::publish_authenticate_registry` with the authenticate
/// functions constant as argument. If the module does not call
/// `publish_authenticate_registry` then it is considered valid.
fn verify_init_publish_authenticate_registry(
    view: &CompiledModule,
    init_code: &[Bytecode],
) -> Result<(), String> {
    let mut found_first_instance_of_call = false;

    // For each instruction in the code, look for the call to
    // `0x2::account::publish_authenticate_registry`
    for (i, instr) in init_code.iter().enumerate() {
        if let Bytecode::CallGeneric(finst_idx) = instr {
            let FunctionInstantiation {
                handle,
                type_parameters: _,
            } = view.function_instantiation_at(*finst_idx);

            let candidate_fn_handle = view.function_handle_at(*handle);
            let candidate_mod_handle = view.module_handle_at(candidate_fn_handle.module);
            let candidate_fn_ident = view.identifier_at(candidate_fn_handle.name);
            if (
                *view.address_identifier_at(candidate_mod_handle.address),
                view.identifier_at(candidate_mod_handle.name),
                candidate_fn_ident,
            ) == (
                IOTA_FRAMEWORK_ADDRESS,
                ACCOUNT_MODULE,
                PUBLISH_AUTHENTICATE_REGISTRY_FN_NAME,
            ) {
                // If we have already found a call to `publish_authenticate_registry` then
                // this is an error since it can only be called once
                if found_first_instance_of_call {
                    return Err(format!(
                        "The 'publish_authenticate_registry' function can only be called once in the 'init' function"
                    ));
                }
                found_first_instance_of_call = true;

                // If the call instruction to `publish_authenticate_registry` is found, then
                // verify that the second-to-last argument a valid constant.

                // check that the second to last argument is the constant at
                // index `expected_const_idx`
                let pos = i.checked_sub(2).ok_or_else(|| {
                        format!(
                            "Expected at least 2 instructions preceding the 'publish_authenticate_registry' call instruction"
                        )
                    })?;
                let second_to_last_instr = init_code.get(pos).ok_or_else(|| {
                        format!(
                            "Expected at least 2 instructions preceding the 'publish_authenticate_registry' call instruction"
                        )
                    })?;
                // check that it is a LdConst instruction with the expected const index found
                // above
                match second_to_last_instr {
                    Bytecode::LdConst(actual_idx) => {
                        verify_authenticate_functions_const(view, *actual_idx)?
                    }
                    other => {
                        return Err(format!(
                            "Expected the argument to 'publish_authenticate_registry' to be a constant, but found instruction '{:?}'",
                            other
                        ));
                    }
                }
            }
        }
    }

    Ok(())
}
