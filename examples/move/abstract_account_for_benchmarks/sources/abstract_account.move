// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module abstract_account_for_benchmarks::abstract_account;

use iota::account;
use iota::authenticator_function;
use iota::dynamic_field;
use iota::ed25519;
use iota::package_metadata::PackageMetadataV1;
use std::ascii;
use iota::hex::decode;

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

/// Ed25519 signature authenticator.
#[authenticator]
public fun authenticate_ed25519(
    account: &AbstractAccount,
    signature: vector<u8>,
    _: &AuthContext,
    ctx: &TxContext,
) {
    // Check the signature.
    assert!(
        ed25519::ed25519_verify(
            &decode(signature),
            account.borrow_public_key(),
            ctx.digest(),
        ),
        0,
    );
}

/// Ed25519 signature authenticator.
#[authenticator]
public fun authenticate_ed25519_heavy(
    account: &AbstractAccount,
    signature: vector<u8>,
    _: &AuthContext,
    ctx: &TxContext,
) {
    let mut i = 0;
    while (i < 5) {
         ed25519::ed25519_verify(
            &decode(signature),
            account.borrow_public_key(),
            ctx.digest(),
        );
        i = i + 1;
    };
}

#[authenticator]
public fun authenticate_hello_world(
    _account: &AbstractAccount,
    msg: ascii::String,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {
    assert!(msg == ascii::string(b"HelloWorld"), 0);
}

/// Object we will pass as extra input to measure storage reads.
public struct BenchObject has key {
    id: UID,
    counter: u64,
}

public entry fun create_bench_objects(objects_amount: u64, is_shared: bool, ctx: &mut TxContext) {
    let mut i = 0;
    while (i < objects_amount) {
        let o = BenchObject { id: object::new(ctx), counter: 0 };
        if (is_shared) {
            transfer::share_object(o);
        } else {
            transfer::freeze_object(o);
        };
        i = i + 1;
    }
}

public fun touch(obj: &mut BenchObject, _ctx: &mut TxContext) {
    obj.counter = obj.counter + 1;
}

#[authenticator]
public fun authenticate_max_args_125(
    _account: &AbstractAccount,
    _o1: &BenchObject,  _o2: &BenchObject,  _o3: &BenchObject,  _o4: &BenchObject,
    _o5: &BenchObject,  _o6: &BenchObject,  _o7: &BenchObject,  _o8: &BenchObject,
    _o9: &BenchObject,  _o10: &BenchObject, _o11: &BenchObject, _o12: &BenchObject,
    _o13: &BenchObject, _o14: &BenchObject, _o15: &BenchObject, _o16: &BenchObject,
    _o17: &BenchObject,  _o18: &BenchObject, _o19: &BenchObject, _o20: &BenchObject,
    _o21: &BenchObject,  _o22: &BenchObject, _o23: &BenchObject, _o24: &BenchObject,
    _o25: &BenchObject,  _o26: &BenchObject, _o27: &BenchObject, _o28: &BenchObject,
    _o29: &BenchObject,  _o30: &BenchObject, _o31: &BenchObject, _o32: &BenchObject,
    _o33: &BenchObject,  _o34: &BenchObject, _o35: &BenchObject, _o36: &BenchObject,
    _o37: &BenchObject,  _o38: &BenchObject, _o39: &BenchObject, _o40: &BenchObject,
    _o41: &BenchObject,  _o42: &BenchObject, _o43: &BenchObject, _o44: &BenchObject,
    _o45: &BenchObject,  _o46: &BenchObject, _o47: &BenchObject, _o48: &BenchObject,
    _o49: &BenchObject,  _o50: &BenchObject, _o51: &BenchObject, _o52: &BenchObject,
    _o53: &BenchObject,  _o54: &BenchObject, _o55: &BenchObject, _o56: &BenchObject,
    _o57: &BenchObject,  _o58: &BenchObject, _o59: &BenchObject, _o60: &BenchObject,
    _o61: &BenchObject,  _o62: &BenchObject, _o63: &BenchObject, _o64: &BenchObject,
    _o65: &BenchObject,  _o66: &BenchObject, _o67: &BenchObject, _o68: &BenchObject,
    _o69: &BenchObject,  _o70: &BenchObject, _o71: &BenchObject, _o72: &BenchObject,
    _o73: &BenchObject,  _o74: &BenchObject, _o75: &BenchObject, _o76: &BenchObject,
    _o77: &BenchObject,  _o78: &BenchObject, _o79: &BenchObject, _o80: &BenchObject,
    _o81: &BenchObject,  _o82: &BenchObject, _o83: &BenchObject, _o84: &BenchObject,
    _o85: &BenchObject,  _o86: &BenchObject, _o87: &BenchObject, _o88: &BenchObject,
    _o89: &BenchObject,  _o90: &BenchObject, _o91: &BenchObject, _o92: &BenchObject,
    _o93: &BenchObject,  _o94: &BenchObject, _o95: &BenchObject, _o96: &BenchObject,
    _o97: &BenchObject,  _o98: &BenchObject, _o99: &BenchObject, _o100: &BenchObject,
    _o101: &BenchObject, _o102: &BenchObject, _o103: &BenchObject, _o104: &BenchObject,
    _o105: &BenchObject, _o106: &BenchObject, _o107: &BenchObject, _o108: &BenchObject,
    _o109: &BenchObject, _o110: &BenchObject, _o111: &BenchObject, _o112: &BenchObject,
    _o113: &BenchObject, _o114: &BenchObject, _o115: &BenchObject, _o116: &BenchObject,
    _o117: &BenchObject, _o118: &BenchObject, _o119: &BenchObject, _o120: &BenchObject,
    _o121: &BenchObject, _o122: &BenchObject,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}
