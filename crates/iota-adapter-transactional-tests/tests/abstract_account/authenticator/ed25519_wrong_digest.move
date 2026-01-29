// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// ed25519 authentication fails due to wrong digest

//# init --addresses test=0x0 abstract_account_with_pub_key=0x0 --accounts A

//# publish-dependencies --paths crates/iota-adapter-transactional-tests/data/account_abstraction/abstract_account_with_pub_key.move

//# publish --sender A --dependencies abstract_account_with_pub_key
module test::authenticate;

use abstract_account_with_pub_key::abstract_account::AbstractAccount;
use iota::ed25519;

/// Ed25519 signature authenticator.
#[authenticator]
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
            account.borrow_public_key(),
            &digest,
        ),
        0,
    );
}

//# init-abstract-account --sender A --package-metadata object(3,1) --inputs "authenticate" "authenticate_ed25519" x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88" --create-function abstract_account_with_pub_key::abstract_account::create --account-type abstract_account_with_pub_key::abstract_account::AbstractAccount

//# view-object 4,0

//# abstract --account immshared(4,0) --auth-inputs x"cce72947906dbae4c166fc01fd096432784032be43db540909bc901dbc057992b4d655ca4f4355cf0868e1266baacf6919902969f063e74162f8f04bc4056105" x"315f5bdb76d078c43b8ac0064e4a0164612b1fce77c869345bfc94c10000edd3" --ptb-inputs 100 @A
//> 0: SplitCoins(Gas, [Input(0)]);
//> 1: TransferObjects([Result(0)], Input(1));
