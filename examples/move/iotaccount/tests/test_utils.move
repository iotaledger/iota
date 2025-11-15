// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iotaccount::test_utils;

use iota::account;
use iotaccount::iotaccount::{IOTAccount, builder, share};

public fun create_iotaccount_for_testing(scenario: &mut iota::test_scenario::Scenario): address {
    let ctx = iota::test_scenario::ctx(scenario);

    let authenticator = create_authenticator_info_v1_for_testing();
    let authenticator_metadata = create_authenticator_info_metadata_v1_for_testing(authenticator);

    let account = builder(authenticator_metadata, ctx)
        .add_dynamic_field(b"SomeData".to_ascii_string(), 3u8)
        .finish();
    let account_address = account.account_address();

    share(account);

    account_address
}

public fun create_authenticator_info_v1_for_testing(): account::AuthenticatorInfoV1 {
    // The exact values don't matter in these tests.
    account::create_auth_info_v1_for_testing(
        @0x1,
        std::ascii::string(b"iotaccount"),
        std::ascii::string(b"authenticate"),
    )
}

public fun create_authenticator_info_metadata_v1_for_testing(
    authenticator_info: account::AuthenticatorInfoV1,
): account::AuthenticatorInfoMetadataV1 {
    account::create_auth_info_metadata_v1_for_testing(
        authenticator_info,
        std::type_name::get<IOTAccount>(),
    )
}
