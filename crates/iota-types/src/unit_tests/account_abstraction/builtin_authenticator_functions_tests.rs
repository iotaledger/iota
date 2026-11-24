// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_types::ObjectId;

use crate::{
    IOTA_FRAMEWORK_PACKAGE_ID, IOTA_SYSTEM_PACKAGE_ID,
    account_abstraction::{
        authenticator_function::AuthenticatorFunctionRefV1,
        builtin_authenticator_functions::{
            BUILTIN_AUTHENTICATOR_FUNCTION_V1_NAME, BUILTIN_AUTHENTICATOR_FUNCTIONS_MODULE_NAME,
            builtin_authenticator_function_ref_v1, is_builtin_authenticator_function_ref,
        },
    },
};

// === is_builtin_authenticator_function_ref() ===

#[test]
fn builtin_ref_is_recognized() {
    assert!(is_builtin_authenticator_function_ref(
        &builtin_authenticator_function_ref_v1()
    ));
}

#[test]
fn builtin_ref_not_recognized_for_wrong_package() {
    let reference = make_ref(
        IOTA_SYSTEM_PACKAGE_ID,
        BUILTIN_AUTHENTICATOR_FUNCTIONS_MODULE_NAME.as_str(),
        BUILTIN_AUTHENTICATOR_FUNCTION_V1_NAME,
    );
    assert!(!is_builtin_authenticator_function_ref(&reference));
}

#[test]
fn builtin_ref_not_recognized_for_wrong_module() {
    let reference = make_ref(
        IOTA_FRAMEWORK_PACKAGE_ID,
        "other_module",
        BUILTIN_AUTHENTICATOR_FUNCTION_V1_NAME,
    );
    assert!(!is_builtin_authenticator_function_ref(&reference));
}

#[test]
fn builtin_ref_not_recognized_for_unknown_function() {
    let reference = make_ref(
        IOTA_FRAMEWORK_PACKAGE_ID,
        BUILTIN_AUTHENTICATOR_FUNCTIONS_MODULE_NAME.as_str(),
        "ed25519_authenticator_v1",
    );
    assert!(!is_builtin_authenticator_function_ref(&reference));
}

// === builtin_authenticator_function_ref_v1() ===

#[test]
fn builtin_ref_has_correct_fields() {
    let reference = builtin_authenticator_function_ref_v1();
    assert_eq!(reference.package, IOTA_FRAMEWORK_PACKAGE_ID);
    assert_eq!(
        reference.module,
        BUILTIN_AUTHENTICATOR_FUNCTIONS_MODULE_NAME.as_str()
    );
    assert_eq!(reference.function, BUILTIN_AUTHENTICATOR_FUNCTION_V1_NAME);
}

// === Helpers ===

fn make_ref(package: ObjectId, module: &str, function: &str) -> AuthenticatorFunctionRefV1 {
    AuthenticatorFunctionRefV1 {
        package,
        module: module.to_string(),
        function: function.to_string(),
    }
}
