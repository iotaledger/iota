// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::cell::Cell;

use iota_sdk_types::ObjectId;

use crate::{
    account_abstraction::authenticator_function::{
        AuthenticatorFunctionRef, AuthenticatorFunctionRefV1, extract_auth_fun_refs,
    },
    base_types::IotaAddress,
};

#[test]
fn both_auth_fun_refs_queried_when_sender_differs_from_gas_owner() {
    let sender = IotaAddress::from([1; 32]);
    let gas_owner = IotaAddress::from([2; 32]);
    let sender_authenticator_function_ref = authenticator_function_ref_v1("sender_auth_fun");
    let sponsor_authenticator_function_ref = authenticator_function_ref_v1("sponsor_auth_fun");
    let call_count = Cell::new(0u32);

    let (sender_auth_fun_ref, sponsor_auth_fun_ref) =
        extract_auth_fun_refs(sender, gas_owner, |a| {
            call_count.set(call_count.get() + 1);

            if a == sender {
                Some(sender_authenticator_function_ref.clone())
            } else if a == gas_owner {
                Some(sponsor_authenticator_function_ref.clone())
            } else {
                None
            }
        });

    assert_eq!(
        call_count.get(),
        2,
        "find_ref must be called for both sender and gas_owner"
    );

    assert_eq!(sender_auth_fun_ref, Some(sender_authenticator_function_ref));
    assert_eq!(
        sponsor_auth_fun_ref,
        Some(sponsor_authenticator_function_ref)
    );
}

#[test]
fn sponsor_auth_fun_ref_not_queried_when_sender_is_gas_owner() {
    let sender = IotaAddress::from([1; 32]);
    let sender_authenticator_function_ref = authenticator_function_ref_v1("sender_auth_fun");
    let call_count = Cell::new(0u32);

    let (sender_auth_fun_ref, sponsor_auth_fun_ref) = extract_auth_fun_refs(sender, sender, |a| {
        call_count.set(call_count.get() + 1);
        assert_eq!(a, sender);
        Some(sender_authenticator_function_ref.clone())
    });

    assert_eq!(
        call_count.get(),
        1,
        "find_ref must not be called for sponsor when sender == gas_owner"
    );

    assert_eq!(sender_auth_fun_ref, Some(sender_authenticator_function_ref));
    assert_eq!(sponsor_auth_fun_ref, None);
}

fn authenticator_function_ref_v1(function: &str) -> AuthenticatorFunctionRef {
    AuthenticatorFunctionRef::V1(AuthenticatorFunctionRefV1 {
        package: ObjectId::from([1u8; 32]),
        module: "mod".to_string(),
        function: function.to_string(),
    })
}
