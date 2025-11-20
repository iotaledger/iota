// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iotaccount::test_utils;

use iota::account;
use iota::package_metadata;
use iota::test_utils;
use iotaccount::iotaccount::{IOTAccount, builder, share};
use std::ascii;
use std::type_name::{Self, TypeName};

public fun create_iotaccount_for_testing(scenario: &mut iota::test_scenario::Scenario): address {
    let ctx = iota::test_scenario::ctx(scenario);

    let package_metadata = create_package_metadata_for_testing();

    let account = builder(
        &package_metadata,
        default_module_name(),
        default_function_name(),
        ctx,
    )
        .add_dynamic_field(b"SomeData".to_ascii_string(), 3u8)
        .finish();
    let account_address = account.account_address();

    share(account);
    test_utils::destroy(package_metadata);

    account_address
}

public fun default_package(): address {
    @0x1
}

public fun default_module_name(): ascii::String {
    ascii::string(b"iotaccount")
}

public fun default_function_name(): ascii::String {
    ascii::string(b"authenticate")
}

public fun default_account_type(): TypeName {
    type_name::get<IOTAccount>()
}

public fun create_authenticator_info_v1_for_testing(): account::AuthenticatorInfoV1 {
    // The exact values don't matter in these tests.
    account::create_auth_info_v1_for_testing(
        default_package(),
        default_module_name(),
        default_function_name(),
    )
}

public fun create_package_metadata_for_testing(): package_metadata::PackageMetadataV1 {
    package_metadata::create_package_metadata_v1_for_testing_one_authenticator(
        default_package().to_id(),
        default_module_name(),
        default_function_name(),
        default_account_type(),
    )
}
