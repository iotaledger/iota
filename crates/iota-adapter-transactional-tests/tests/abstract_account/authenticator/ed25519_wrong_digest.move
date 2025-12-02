// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// ed25519 authentication fails due to wrong digest

//# init --addresses test=0x0 --accounts A

//# publish --sender A
module test::authenticate;

use iota::account::{Self, AuthenticatorInfoV1};
use iota::auth_context::AuthContext;
use iota::dynamic_field;
use iota::ed25519;

public struct AbstractAccount has key {
    id: UID,
}

public struct OwnerPublicKey has copy, drop, store {}

public fun create(
    public_key: vector<u8>,
    authenticator: AuthenticatorInfoV1<AbstractAccount>,
    ctx: &mut TxContext,
): address {
    let mut account = AbstractAccount { id: object::new(ctx) };
    let authenticator_compatibility_proof = account::check_auth_info_v1_compatibility(
        &account,
        authenticator,
    );
    account::attach_auth_info_v1(&mut account.id, authenticator_compatibility_proof);
    dynamic_field::add(&mut account.id, OwnerPublicKey {}, public_key);
    let account_address = object::id_address(&account);
    iota::transfer::share_object(account);
    account_address
}

/// Ed25519 signature authenticator.
public fun authenticate_ed25519(
    account: &AbstractAccount,
    signature: vector<u8>,
    digest: vector<u8>,
    _: &AuthContext,
    _ctx: &TxContext,
) {
    // Check the signature.
    assert!(
        ed25519::ed25519_verify(
            &signature,
            dynamic_field::borrow(&account.id, OwnerPublicKey {}),
            &digest,
        ),
        0,
    );
}

//# init-abstract-acc --sender A test authenticate authenticate_ed25519 --inputs x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88" --custom

//# view-object 2,2

//# abstract --account immshared(2,2) --auth-inputs x"cce72947906dbae4c166fc01fd096432784032be43db540909bc901dbc057992b4d655ca4f4355cf0868e1266baacf6919902969f063e74162f8f04bc4056105" x"315f5bdb76d078c43b8ac0064e4a0164612b1fce77c869345bfc94c10000edd3" --ptb-inputs 100 @A
//> 0: SplitCoins(Gas, [Input(0)]);
//> 1: TransferObjects([Result(0)], Input(1));
