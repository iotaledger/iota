// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module abstract_account::abstract_keyed_account;

use abstract_account::abstract_account::{Self, AbstractAccount};
use abstract_account::basic_keyed_account;
use iota::account::AuthenticatorInfoV1;

#[allow(lint(self_transfer))]
/// Use an AbstractAccountBuilder and create an Abstract Account with basic keys.
public fun create_abstract_keyed_account(
    public_key: vector<u8>,
    authenticator: AuthenticatorInfoV1,
    ctx: &mut TxContext,
): AbstractAccount {
    let (mut aa, cap) = abstract_account::builder(authenticator, ctx).finish();
    let uid = abstract_account::uid_mut(&mut aa, &cap);
    basic_keyed_account::create(uid, public_key);
    transfer::public_transfer(cap, ctx.sender());
    aa
}
