// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module simple_abstract_account::abstract_account;

use iota::package_metadata::PackageMetadataV1;
use iota::account::{Self, AuthenticatorInfoV1};
use std::ascii;

public struct AbstractAccount has key {
    id: UID,
}

public fun create(
    package_metadata: &PackageMetadataV1,
    module_name: ascii::String,
    function_name: ascii::String,
    ctx: &mut TxContext,
): address {
    let authenticator = account::create_auth_info_v1<AbstractAccount>(
        package_metadata,
        module_name,
        function_name,
    );

    create_with_auth_info(authenticator, ctx)
}

public fun create_with_auth_info(
    authenticator: AuthenticatorInfoV1<AbstractAccount>,
    ctx: &mut TxContext,
): address {

    let account = AbstractAccount { id: object::new(ctx) };

    let account_address = object::id_address(&account);

    account::create_account_v1(account, authenticator);

    account_address
}