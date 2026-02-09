// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module abstract_account_with_pub_key::abstract_account;

use iota::account;
use iota::auth_context::AuthContext;
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
    let authenticator = account::create_auth_info_v1<AbstractAccount>(
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
    while (i < 250) {
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
}

public entry fun create_125_bench_objects(ctx: &mut TxContext) {
    let mut i = 0;
    while (i < 125) {
        let o = BenchObject { id: object::new(ctx) };
        transfer::freeze_object(o);
        i = i + 1;
    }
}

public entry fun create_252_bench_objects(ctx: &mut TxContext) {
    let mut i = 0;
    while (i < 252) {
        let o = BenchObject { id: object::new(ctx) };
        transfer::freeze_object(o);
        i = i + 1;
    }
}

#[authenticator]
public fun authenticate_max_args_128(
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
    _o121: &BenchObject, _o122: &BenchObject, _o123: &BenchObject, _o124: &BenchObject,
    _o125: &BenchObject,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}


#[authenticator]
public fun authenticate_max_args_255(
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
    _o121: &BenchObject, _o122: &BenchObject, _o123: &BenchObject, _o124: &BenchObject,
    _o125: &BenchObject, _o126: &BenchObject, _o127: &BenchObject, _o128: &BenchObject,
    _o129: &BenchObject, _o130: &BenchObject, _o131: &BenchObject, _o132: &BenchObject,
    _o133: &BenchObject, _o134: &BenchObject, _o135: &BenchObject, _o136: &BenchObject,
    _o137: &BenchObject, _o138: &BenchObject, _o139: &BenchObject, _o140: &BenchObject,
    _o141: &BenchObject, _o142: &BenchObject, _o143: &BenchObject, _o144: &BenchObject,
    _o145: &BenchObject, _o146: &BenchObject, _o147: &BenchObject, _o148: &BenchObject,
    _o149: &BenchObject, _o150: &BenchObject, _o151: &BenchObject, _o152: &BenchObject,
    _o153: &BenchObject, _o154: &BenchObject, _o155: &BenchObject, _o156: &BenchObject,
    _o157: &BenchObject, _o158: &BenchObject, _o159: &BenchObject, _o160: &BenchObject,
    _o161: &BenchObject, _o162: &BenchObject, _o163: &BenchObject, _o164: &BenchObject,
    _o165: &BenchObject, _o166: &BenchObject, _o167: &BenchObject, _o168: &BenchObject,
    _o169: &BenchObject, _o170: &BenchObject, _o171: &BenchObject, _o172: &BenchObject,
    _o173: &BenchObject, _o174: &BenchObject, _o175: &BenchObject, _o176: &BenchObject,
    _o177: &BenchObject, _o178: &BenchObject, _o179: &BenchObject, _o180: &BenchObject,
    _o181: &BenchObject, _o182: &BenchObject, _o183: &BenchObject, _o184: &BenchObject,
    _o185: &BenchObject, _o186: &BenchObject, _o187: &BenchObject, _o188: &BenchObject,
    _o189: &BenchObject, _o190: &BenchObject, _o191: &BenchObject, _o192: &BenchObject,
    _o193: &BenchObject, _o194: &BenchObject, _o195: &BenchObject, _o196: &BenchObject,
    _o197: &BenchObject, _o198: &BenchObject, _o199: &BenchObject, _o200: &BenchObject,
    _o201: &BenchObject, _o202: &BenchObject, _o203: &BenchObject, _o204: &BenchObject,
    _o205: &BenchObject, _o206: &BenchObject, _o207: &BenchObject, _o208: &BenchObject,
    _o209: &BenchObject, _o210: &BenchObject, _o211: &BenchObject, _o212: &BenchObject,
    _o213: &BenchObject, _o214: &BenchObject, _o215: &BenchObject, _o216: &BenchObject,
    _o217: &BenchObject, _o218: &BenchObject, _o219: &BenchObject, _o220: &BenchObject,
    _o221: &BenchObject, _o222: &BenchObject, _o223: &BenchObject, _o224: &BenchObject,
    _o225: &BenchObject, _o226: &BenchObject, _o227: &BenchObject, _o228: &BenchObject,
    _o229: &BenchObject, _o230: &BenchObject, _o231: &BenchObject, _o232: &BenchObject,
    _o233: &BenchObject, _o234: &BenchObject, _o235: &BenchObject, _o236: &BenchObject,
    _o237: &BenchObject, _o238: &BenchObject, _o239: &BenchObject, _o240: &BenchObject,
    _o241: &BenchObject, _o242: &BenchObject, _o243: &BenchObject, _o244: &BenchObject,
    _o245: &BenchObject, _o246: &BenchObject, _o247: &BenchObject, _o248: &BenchObject,
    _o249: &BenchObject, _o250: &BenchObject, _o251: &BenchObject, _o252: &BenchObject,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}
