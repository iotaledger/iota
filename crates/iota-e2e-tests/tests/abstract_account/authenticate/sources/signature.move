// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module authenticate::signature;

use authenticate::account::{Self, Account};
use iota::auth_context::AuthContext;

// PASS
public fun minimally_viable_auth_function(
    _account: &Account,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

// FAIL
#[allow(unused_function)]
fun has_to_be_public_auth_function(_account: &Account, _auth_ctx: &AuthContext, _ctx: &TxContext) {}

// FAIL
public fun without_account(_auth_ctx: &AuthContext, _ctx: &TxContext) {}

// FAIL
public fun without_auth_context(_account: &Account, _ctx: &TxContext) {}

// FAIL
public fun without_tx_context(_account: &Account, _auth_ctx: &AuthContext) {}

// FAIL
public fun account_cant_be_value(account: Account, _auth_ctx: &AuthContext, _ctx: &TxContext) {
    account::destroy(account);
}

// FAIL
public fun auth_context_cant_be_value(
    _account: &Account,
    _auth_ctx: AuthContext,
    _ctx: &TxContext,
) {}

// FAIL
public fun tx_context_cant_be_value(_account: &Account, _auth_ctx: &AuthContext, _ctx: TxContext) {}

// FAIL
public fun account_cant_be_mutable_ref(
    _account: &mut Account,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

// FAIL
public fun auth_context_cant_be_mutable_ref(
    _account: &Account,
    _auth_ctx: &mut AuthContext,
    _ctx: &TxContext,
) {}

// FAIL
public fun tx_context_cant_be_mutable_ref(
    _account: &Account,
    _auth_ctx: &AuthContext,
    _ctx: &mut TxContext,
) {}

// FAIL
public fun account_isnt_struct(_account: u64, _auth_ctx: &AuthContext, _ctx: &TxContext) {}

// FAIL
public fun auth_context_isnt_struct(_account: &Account, _auth_ctx: u64, _ctx: &TxContext) {}

// FAIL
public fun tx_context_isnt_struct(_account: &Account, _auth_ctx: &AuthContext, _ctx: u64) {}

// PASS
public fun arg_value(_account: &Account, _val: u8, _auth_ctx: &AuthContext, _ctx: &TxContext) {}

// PASS
public fun arg_mutable_value(
    _account: &Account,
    mut _val: u8,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

// FAIL
public fun with_signer(_account: &Account, _s: signer, _auth_ctx: &AuthContext, _ctx: &TxContext) {}
