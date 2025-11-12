// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// simple authenticate test for abstract accounts with sponsorship

//# init --addresses test=0x0 --accounts A

//# publish --sender A
module test::abstract_account;

use iota::account::{Self, AuthenticatorInfoV1};
use iota::auth_context::AuthContext;
use iota::dynamic_field;

#[error(code = 0)]
const ETransactionSenderIsNotTheAccount: vector<u8> = b"Transaction must be signed by the account.";

public struct AbstractAccount has key {
    id: UID,
}

/// A dynamic field key for the account owner public key.
public struct OwnerPublicKey has copy, drop, store {}

public fun create(public_key: vector<u8>, authenticator: AuthenticatorInfoV1<AbstractAccount>, ctx: &mut TxContext) {
    let mut account = AbstractAccount { id: object::new(ctx) };
    let authenticator_compatibility_proof = account::check_auth_info_v1_compatibility(
        &account,
        authenticator,
    );

    account::attach_auth_info_v1(&mut account.id, authenticator_compatibility_proof);
    dynamic_field::add(&mut account.id, OwnerPublicKey {}, public_key);
    iota::transfer::share_object(account);
}

public fun ensure_tx_sender_is_account(self: &AbstractAccount, ctx: &TxContext) {
    assert!(self.id.uid_to_address() == ctx.sender(), ETransactionSenderIsNotTheAccount);
}

public fun authenticate(account: &AbstractAccount, _auth_ctx: &AuthContext, ctx: &TxContext) {
    ensure_tx_sender_is_account(account, ctx);
}

//# programmable --sender A --inputs x"10" @test "abstract_account" "authenticate"
//> 0: iota::account::create_auth_info_v1<test::abstract_account::AbstractAccount>(Input(1), Input(2), Input(3));
//> 1: test::abstract_account::create(Input(0), Result(0));

//# view-object 2,2

//# set-address a_account object(2,2)

//# abstract --account immshared(2,2) --sponsor A --ptb-inputs 100 @A
//> 0: SplitCoins(Gas, [Input(0)]);
//> 1: TransferObjects([Result(0)], Input(1));

//# view-object 5,0
