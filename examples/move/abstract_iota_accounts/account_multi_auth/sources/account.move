// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Example account module with an authenticator that accepts multiple argument
/// types including nested vectors, strings, options, and object references to
/// exercise the full range of auth-call-args parsing.
module account_multi_auth::account;

use std::string::String;

use iota::clock::{Self, Clock};
use iota::package_metadata::PackageMetadataV1;
use iota::account;
use iota::authenticator_function;

public struct Account has key, store {
    id: UID,
}

public struct ACCOUNT has drop {}

fun init(_otw: ACCOUNT, ctx: &mut TxContext) {
    transfer::public_share_object(Account {
        id: object::new(ctx),
    });
}

public fun link_auth(
    account: Account,
    package: &PackageMetadataV1,
    module_name: std::ascii::String,
    function_name: std::ascii::String,
) {
    let authenticator = authenticator_function::create_auth_function_ref_v1<Account>(
        package,
        module_name,
        function_name,
    );
    account::create_account_v1<Account>(account, authenticator);
}

/// An authenticator function that validates multiple argument types:
/// - `magic_number`: a u64 that must equal 42
/// - `secret`: a vector<u8> that must equal 0xCAFE
/// - `nested`: a vector<vector<u8>> that must have exactly 2 elements,
///   where the first is 0xAA and the second is 0xBBCC
/// - `label`: a String that must equal "test"
/// - `optional`: an Option<vector<u8>> that must be Some([0xDE, 0xAD])
/// - `clock`: an immutable &Clock reference (proves object refs work)
#[authenticator]
public fun authenticate(
    _account: &Account,
    magic_number: u64,
    secret: vector<u8>,
    nested: vector<vector<u8>>,
    label: String,
    optional: Option<vector<u8>>,
    clock: &Clock,
    _auth_ctx: &iota::auth_context::AuthContext,
    _ctx: &TxContext,
) {
    // Validate magic_number
    assert!(magic_number == 42, 0);

    // Validate secret == 0xCAFE
    assert!(secret.length() == 2, 1);
    assert!(*secret.borrow(0) == 0xCA, 2);
    assert!(*secret.borrow(1) == 0xFE, 3);

    // Validate nested has 2 elements
    assert!(nested.length() == 2, 4);

    // First inner vector must be [0xAA]
    let first = nested.borrow(0);
    assert!(first.length() == 1, 5);
    assert!(*first.borrow(0) == 0xAA, 6);

    // Second inner vector must be [0xBB, 0xCC]
    let second = nested.borrow(1);
    assert!(second.length() == 2, 7);
    assert!(*second.borrow(0) == 0xBB, 8);
    assert!(*second.borrow(1) == 0xCC, 9);

    // Validate label == "test"
    assert!(label == std::string::utf8(b"test"), 10);

    // Validate optional is Some containing [0xDE, 0xAD]
    assert!(optional.is_some(), 11);
    let opt_val = optional.borrow();
    assert!(opt_val.length() == 2, 12);
    assert!(*opt_val.borrow(0) == 0xDE, 13);
    assert!(*opt_val.borrow(1) == 0xAD, 14);

    // Validate clock reference works (proves object ref args function)
    let _ts = clock::timestamp_ms(clock);
}
