// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// authenticate test for abstract accounts with receiving argument

//# init --addresses test=0x0 --accounts A

//# publish --sender A
module test::abstract_account;

use iota::account::{Self, AuthenticatorInfoV1};
use iota::auth_context::AuthContext;
use iota::coin::Coin;
use iota::dynamic_field;
use iota::iota::IOTA;

#[error(code = 0)]
const ETransactionSenderIsNotTheAccount: vector<u8> = b"Transaction must be signed by the account.";

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

public fun authenticate_receive_coin(
    account: &AbstractAccount,
    _coin: transfer::Receiving<Coin<IOTA>>,
    _: &AuthContext,
    ctx: &TxContext,
) {
    ensure_tx_sender_is_account(account, ctx);
}

//# programmable --sender A --inputs x"10" @test "abstract_account" "authenticate_receive_coin"
//> 0: iota::account::create_auth_info_v1(Input(1), Input(2), Input(3));
//> 1: test::abstract_account::create(Input(0), Result(0));
