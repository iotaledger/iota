// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iotaccount::test_utils;

use iota::authenticator_function::{Self, AuthenticatorFunctionRefV1};
use iotaccount::iotaccount::{builder, IOTAccount};

public fun create_iotaccount_for_testing(scenario: &mut iota::test_scenario::Scenario): address {
    let ctx = iota::test_scenario::ctx(scenario);

    let authenticator = create_authenticator_function_ref_v1_for_testing();

    builder(authenticator, ctx).add_dynamic_field(b"SomeData".to_ascii_string(), 3u8).build()
}

public fun create_authenticator_function_ref_v1_for_testing(): AuthenticatorFunctionRefV1<
    IOTAccount,
> {
    // The exact values don't matter in these tests.
    authenticator_function::create_auth_function_ref_v1_for_testing(
        @0x1,
        std::ascii::string(b"iotaccount"),
        std::ascii::string(b"authenticate"),
    )
}
