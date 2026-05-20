// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module simple_abstract_account::abstract_account;

use iota::account;
use iota::authenticator_function::{Self, AuthenticatorFunctionRefV1};
use iota::package_metadata::PackageMetadataV2;
use std::ascii;

public struct AbstractAccount has key {
    id: UID,
}

public fun create(
    package_metadata: &PackageMetadataV2,
    module_name: ascii::String,
    function_name: ascii::String,
    ctx: &mut TxContext,
): address {
    let authenticator = authenticator_function::create_auth_function_ref_from_package_metadata_v2<
        AbstractAccount,
    >(
        package_metadata,
        module_name,
        function_name,
    );

    create_with_auth_function_ref(authenticator, ctx)
}

public fun create_immutable(
    package_metadata: &PackageMetadataV2,
    module_name: ascii::String,
    function_name: ascii::String,
    ctx: &mut TxContext,
): address {
    let authenticator = authenticator_function::create_auth_function_ref_from_package_metadata_v2<
        AbstractAccount,
    >(
        package_metadata,
        module_name,
        function_name,
    );

    create_immutable_with_auth_function_ref(authenticator, ctx)
}

public fun create_with_auth_function_ref(
    authenticator: AuthenticatorFunctionRefV1<AbstractAccount>,
    ctx: &mut TxContext,
): address {
    let account = AbstractAccount { id: object::new(ctx) };

    let account_address = object::id_address(&account);

    account::create_account_v1(account, authenticator);

    account_address
}

public fun create_immutable_with_auth_function_ref(
    authenticator: AuthenticatorFunctionRefV1<AbstractAccount>,
    ctx: &mut TxContext,
): address {
    let account = AbstractAccount { id: object::new(ctx) };

    let account_address = object::id_address(&account);

    account::create_immutable_account_v1(account, authenticator);

    account_address
}

public fun rotate_auth_function_ref(
    account: &mut AbstractAccount,
    package_metadata: &PackageMetadataV2,
    module_name: ascii::String,
    function_name: ascii::String,
): AuthenticatorFunctionRefV1<AbstractAccount> {
    let authenticator = authenticator_function::create_auth_function_ref_from_package_metadata_v2<
        AbstractAccount,
    >(
        package_metadata,
        module_name,
        function_name,
    );

    account::rotate_auth_function_ref_v1(account, authenticator)
}
