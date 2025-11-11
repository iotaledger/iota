// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// ed25519 authentication fails due to wrong digest

//# init --addresses test=0x0 --accounts A

//# publish --sender A
module test::abstract_account;

use iota::account::{Self, AuthenticatorInfoV1};
use iota::auth_context::AuthContext;
use iota::dynamic_field;
use iota::ed25519;

#[error(code = 0)]
const ETransactionSenderIsNotTheAccount: vector<u8> = b"Transaction must be signed by the account.";

#[error(code = 1)]
const EEd25519VerificationFailed: vector<u8> = b"Ed25519 authenticator verification failed.";

public struct AbstractAccount has key {
    id: UID,
}

/// A dynamic field key for the account owner public key.
public struct OwnerPublicKey has copy, drop, store {}

public fun create(public_key: vector<u8>, authenticator: AuthenticatorInfoV1, ctx: &mut TxContext) {
    let mut account = AbstractAccount { id: object::new(ctx) };
    account::attach_auth_info_v1(&mut account.id, authenticator);
    dynamic_field::add(&mut account.id, OwnerPublicKey {}, public_key);
    iota::transfer::share_object(account);
}

public fun ensure_tx_sender_is_account(self: &AbstractAccount, ctx: &TxContext) {
    assert!(self.id.uid_to_address() == ctx.sender(), ETransactionSenderIsNotTheAccount);
}

/// Ed25519 signature authenticator.
public fun authenticate_ed25519(
    account: &AbstractAccount,
    signature: vector<u8>,
    digest: vector<u8>,
    _: &AuthContext,
    ctx: &TxContext,
) {
    // Check that the sender of this transaction is the account.
    ensure_tx_sender_is_account(account, ctx);

    // Check the signature.
    assert!(
        ed25519::ed25519_verify(
            &signature,
            dynamic_field::borrow(&account.id, OwnerPublicKey {}),
            &digest,
        ),
        EEd25519VerificationFailed,
    );
}

//# programmable --sender A --inputs x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88" @test "abstract_account" "authenticate_ed25519"
//> 0: iota::account::create_auth_info_v1(Input(1), Input(2), Input(3));
//> 1: test::abstract_account::create(Input(0), Result(0));

//# view-object 2,2

//# set-address a_account object(2,2)

//# programmable --sender A --inputs 7000000000 @a_account
//> 0: SplitCoins(Gas, [Input(0)]);
//> 1: TransferObjects([Result(0)], Input(1));

//# view-object 5,0

//# abstract --gas-payment 5,0 --auth-inputs immshared(2,2) x"cce72947906dbae4c166fc01fd096432784032be43db540909bc901dbc057992b4d655ca4f4355cf0868e1266baacf6919902969f063e74162f8f04bc4056105" x"315f5bdb76d078c43b8ac0064e4a0164612b1fce77c869345bfc94c10000edd3" --ptb-inputs 100 @A
//> 0: SplitCoins(Gas, [Input(0)]);
//> 1: TransferObjects([Result(0)], Input(1));

//# view-object 7,0
