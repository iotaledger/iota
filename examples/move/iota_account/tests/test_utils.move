// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota_account::test_utils;

use iota::account;
use iota_account::iota_account::{builder, share};

#[test_only]
public fun create_iotaccount_for_testing(scenario: &mut iota::test_scenario::Scenario): address {
    let ctx = iota::test_scenario::ctx(scenario);

    let authenticator = create_authenticator_info_v1_for_testing();

    let account = builder(authenticator, ctx)
        .add_regular_field(b"SomeData".to_ascii_string(), 3u8)
        .finish();
    let account_address = account.account_address();

    share(account);

    account_address
}

#[test_only]
public fun create_authenticator_info_v1_for_testing(): account::AuthenticatorInfoV1 {
    // The exact values don't matter in these tests.
    account::create_auth_info_v1_for_testing(
        @0x1,
        std::ascii::string(b"iota_account"),
        std::ascii::string(b"authenticate"),
    )
}
