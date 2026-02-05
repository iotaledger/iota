// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module abstract_account_with_pub_key::abstract_account;

use iota::account;
use iota::authenticator_function;
use iota::dynamic_field;
use iota::package_metadata::PackageMetadataV1;
use std::ascii;

public struct AbstractAccount has key {
    id: UID,
}

public struct OwnerPublicKey has copy, drop, store {}

public fun create(
    package_metadata: &PackageMetadataV1,
    module_name: ascii::String,
    function_name: ascii::String,
    public_key: vector<u8>,
    ctx: &mut TxContext,
): address {
    let authenticator = authenticator_function::create_auth_function_ref_v1<AbstractAccount>(
        package_metadata,
        module_name,
        function_name,
    );

    let mut account = AbstractAccount { id: object::new(ctx) };

    dynamic_field::add(&mut account.id, OwnerPublicKey {}, public_key);

    let account_address = object::id_address(&account);

    account::create_account_v1(account, authenticator);

    account_address
}

public fun borrow_public_key(account: &AbstractAccount): &vector<u8> {
    dynamic_field::borrow(&account.id, OwnerPublicKey {})
}
