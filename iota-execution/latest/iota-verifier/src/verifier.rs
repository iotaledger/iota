// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This module contains the public APIs supported by the bytecode verifier.

use iota_types::{error::ExecutionError, move_package::FnInfoMap};
use move_binary_format::file_format::CompiledModule;
use move_bytecode_verifier_meter::{Meter, dummy::DummyMeter};

use crate::{
    entry_points_verifier, global_storage_access_verifier, id_leak_verifier,
    one_time_witness_verifier, private_generics, runtime_module_metadata, struct_with_key_verifier,
};

/// Helper for a "canonical" verification of a module.
///
/// `enable_mutable_shared_in_authenticator` relaxes the authenticator verifier
/// to accept mutable references to object types (see
/// [`crate::authenticator_verifier::verify_authenticate_func_v1`]); it is gated
/// by the `enable_mutable_shared_in_move_authenticator` protocol feature flag.
pub fn iota_verify_module_metered(
    module: &CompiledModule,
    fn_info_map: &FnInfoMap,
    meter: &mut (impl Meter + ?Sized),
    enable_mutable_shared_in_authenticator: bool,
) -> Result<(), ExecutionError> {
    struct_with_key_verifier::verify_module(module)?;
    global_storage_access_verifier::verify_module(module)?;
    id_leak_verifier::verify_module(module, meter)?;
    private_generics::verify_module(module)?;
    entry_points_verifier::verify_module(module, fn_info_map)?;
    one_time_witness_verifier::verify_module(module, fn_info_map)?;
    runtime_module_metadata::verify_module(module, enable_mutable_shared_in_authenticator)
}

/// Runs the IOTA verifier and checks if the error counts as an IOTA verifier
/// timeout NOTE: this function only check if the verifier error is a timeout
/// All other errors are ignored
pub fn iota_verify_module_metered_check_timeout_only(
    module: &CompiledModule,
    fn_info_map: &FnInfoMap,
    meter: &mut (impl Meter + ?Sized),
    enable_mutable_shared_in_authenticator: bool,
) -> Result<(), ExecutionError> {
    // Checks if the error counts as an IOTA verifier timeout
    if let Err(error) = iota_verify_module_metered(
        module,
        fn_info_map,
        meter,
        enable_mutable_shared_in_authenticator,
    ) {
        if matches!(
            error.kind(),
            iota_sdk_types::ExecutionError::IotaMoveVerificationTimeout
        ) {
            return Err(error);
        }
    }
    // Any other scenario, including a non-timeout error counts as Ok
    Ok(())
}

pub fn iota_verify_module_unmetered(
    module: &CompiledModule,
    fn_info_map: &FnInfoMap,
    enable_mutable_shared_in_authenticator: bool,
) -> Result<(), ExecutionError> {
    iota_verify_module_metered(
        module,
        fn_info_map,
        &mut DummyMeter,
        enable_mutable_shared_in_authenticator,
    )
    .inspect_err(|err| {
        // We must never see timeout error in execution
        debug_assert!(
            !matches!(
                err.kind(),
                iota_sdk_types::ExecutionError::IotaMoveVerificationTimeout
            ),
            "Unexpected timeout error in execution"
        );
    })
}
