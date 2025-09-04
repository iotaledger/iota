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
use iota_types::{
    Identifier,
    error::ExecutionError,
    move_package::{FnInfoMap, is_test_fun},
};
use move_binary_format::file_format::{CompiledModule, DatatypeHandle, SignatureToken};
use move_core_types::{ident_str, identifier::IdentStr};

use crate::{
    account_auth_verifier::verify_authenticate_func,
    one_time_witness_verifier::{verify_no_instantiations, verify_one_time_witness},
    verification_failure,
};

pub const AOTW_PREFIX: &str = "AUTH_"; // authenticate one-time witness prefix

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
pub fn verify_module(
    module: &CompiledModule,
    fn_info_map: &FnInfoMap,
) -> Result<(), ExecutionError> {
    let struct_defs = &module.struct_defs;
    let mut authenticate_one_time_witness_candidates = vec![];
    // find structs that can potentially represent a authenticate one-time witness
    // type
    for def in struct_defs {
        let struct_handle = module.datatype_handle_at(def.struct_handle);
        let struct_name = module.identifier_at(struct_handle.name).as_str();

        // check if the struct name starts with the AOTW_PREFIX
        if struct_name.starts_with(AOTW_PREFIX) {
            if let Ok(field_count) = def.declared_field_count() {
                // checks if the struct is non-native (and if it isn't then that's why unwrap
                // below is safe)
                if field_count == 1 && def.field(0).unwrap().signature.0 == SignatureToken::Bool {
                    // a single boolean field means that we found a authenticate one-time witness
                    // candidate - make sure that the remaining properties hold
                    verify_authenticate_one_time_witness(module, struct_name, struct_handle)
                        .map_err(verification_failure)?;
                    // if we reached this point, it means we have a legitimate one-time witness type
                    // candidate and we have to make sure that both the init function's signature
                    // reflects this and that this type is not instantiated in any authenticate of
                    // the module

                    authenticate_one_time_witness_candidates.push((struct_name, def));
                }
            }
        }
    }
    // If authenticate_one_time_witness_candidates is not empty, then verify that
    // there are not authenticate one-time witness type instantiations in any of
    // the module's functions
    if authenticate_one_time_witness_candidates.is_empty() {
        // no authenticate one-time witness type candidates found - nothing more to
        // verify
        return Ok(());
    }
    for fn_def in &module.function_defs {
        let fn_handle = module.function_handle_at(fn_def.function);
        let fn_name = module.identifier_at(fn_handle.name);
        for &(candidate_name, def) in authenticate_one_time_witness_candidates.iter() {
            // only verify lack of authenticate one-time witness type instantiations if we
            // have a one-time witness type candidate and if instantiation does
            // not happen in test code
            if !is_test_fun(fn_name, module, fn_info_map) {
                verify_no_instantiations(module, fn_def, candidate_name, def)
                    .map_err(verification_failure)?;
            }
        }
    }

    Ok(())
}

// Verifies all required properties of a one-time witness type candidate.
// authenticate one-time witness type name must be the same as a capitalized
// authenticate function name
fn verify_authenticate_one_time_witness(
    module: &CompiledModule,
    candidate_full_name: &str,
    candidate_handle: &DatatypeHandle,
) -> Result<(), String> {
    verify_one_time_witness(module, candidate_full_name, candidate_handle)
        .map_err(|e| format!("function {}", e))?;

    // check that the authenticate OTW name is the same as an authenticate function
    if let Ok(candidate_func_name_ident) =
        Identifier::new(&*candidate_full_name[AOTW_PREFIX.len()..].to_ascii_lowercase())
    {
        verify_authenticate_func(module, candidate_func_name_ident.clone()).map_err(|e| {
            format!(
                "authenticate function '{}' does not conform to the rules: {}",
                candidate_func_name_ident, e
            )
        })?
    } else {
        return Err(format!(
            "Expected the candidate function name to be a valid identifier: {:?}",
            candidate_full_name
        ));
    };

    Ok(())
}
