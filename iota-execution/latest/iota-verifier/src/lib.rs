// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub mod verifier;

pub mod authenticator_verifier;
pub mod entry_points_verifier;
pub mod global_storage_access_verifier;
pub mod id_leak_verifier;
pub mod meter;
pub mod one_time_witness_verifier;
pub mod private_generics;
pub mod runtime_module_metadata;
pub mod struct_with_key_verifier;
pub mod view_function_verifier;

use iota_types::error::{ExecutionError, ExecutionErrorKind};
use move_core_types::{ident_str, identifier::IdentStr, vm_status::StatusCode};

pub const INIT_FN_NAME: &IdentStr = ident_str!("init");
pub const TEST_SCENARIO_MODULE_NAME: &str = "test_scenario";

fn verification_failure(error: String) -> ExecutionError {
    ExecutionError::new_with_source(ExecutionErrorKind::IotaMoveVerificationError, error)
}

fn to_verification_timeout_error(error: String) -> ExecutionError {
    ExecutionError::new_with_source(ExecutionErrorKind::IotaMoveVerificationTimeout, error)
}

/// Runs the Move verifier and checks if the error counts as a Move verifier
/// timeout NOTE: this function only check if the verifier error is a timeout
/// All other errors are ignored
pub fn check_for_verifier_timeout(major_status_code: &StatusCode) -> bool {
    [
        StatusCode::PROGRAM_TOO_COMPLEX,
        // Do we want to make this a substatus of `PROGRAM_TOO_COMPLEX`?
        StatusCode::TOO_MANY_BACK_EDGES,
        StatusCode::REFERENCE_SAFETY_INCONSISTENT,
        StatusCode::BORROWLOC_EXISTS_BORROW_ERROR,
        StatusCode::COPYLOC_EXISTS_BORROW_ERROR,
        StatusCode::DEPRECATED_BYTECODE_FORMAT,
        // Signing always applies the stricter vector borrow rule of
        // `deprecate_global_storage_ops`. Without this, a module that rule
        // rejects is published under the old rule before the flag is set, and
        // fails to load once it is.
        StatusCode::VEC_BORROW_ELEMENT_EXISTS_MUTABLE_BORROW_ERROR,
        // Not `CALL_BORROWED_MUTABLE_REFERENCE_ERROR`: calls get the same check
        // whether `deprecate_global_storage_ops` is set or not, so listing it
        // would only move the rejection from execution to signing, and
        // validators with and without that change would disagree in
        // post-consensus validation.
    ]
    .contains(major_status_code)
}
