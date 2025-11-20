// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module authenticate::signature;

use iota::auth_context::AuthContext;

// Test account struct
public struct Account has key {
    id: UID,
}

// PASS
#[authenticator]
public entry fun minimally_viable_auth_function(
    _account: &Account,
    _actx: &AuthContext,
    _ctx: &TxContext,
) {}

// FAIL
//#[authenticator]
public fun has_to_be_entry_auth_function(
    _account: &Account,
    _actx: &AuthContext,
    _ctx: &TxContext,
) {}

// FAIL
//#[authenticator]
public entry fun without_account(_actx: &AuthContext, _ctx: &TxContext) {}

// FAIL
//#[authenticator]
public entry fun without_auth_context(_account: &Account, _ctx: &TxContext) {}

// FAIL Invalid 'entry' parameter type
//#[authenticator]
//public entry fun without_tx_context(_account: &Account, _actx: &AuthContext) {}

// FAIL
//#[authenticator]
public entry fun account_cant_be_value(account: Account, _actx: &AuthContext, _ctx: &TxContext) {
    let Account { id } = account;
    object::delete(id);
}

// FAIL Invalid 'entry' parameter type
//#[authenticator]
//public entry fun auth_context_cant_be_value(
//    _account: &Account,
//    _actx: AuthContext,
//    _ctx: &TxContext,
//) {}

// FAIL
//#[authenticator]
public entry fun account_cant_be_mutable_ref(
    _account: &mut Account,
    _actx: &AuthContext,
    _ctx: &TxContext,
) {}

// FAIL
//#[authenticator]
public entry fun auth_context_cant_be_mutable_ref(
    _account: &Account,
    _actx: &mut AuthContext,
    _ctx: &TxContext,
) {}

// FAIL
//#[authenticator]
public entry fun tx_context_cant_be_mutable_ref(
    _account: &Account,
    _actx: &AuthContext,
    _ctx: &mut TxContext,
) {}

// FAIL
//#[authenticator]
public entry fun account_isnt_struct(_account: u64, _actx: &AuthContext, _ctx: &TxContext) {}

// FAIL
//#[authenticator]
public entry fun auth_context_isnt_struct(_account: &Account, _actx: u64, _ctx: &TxContext) {}

// FAIL Invalid 'entry' parameter type
//#[authenticator]
//public entry fun tx_context_isnt_struct(_account: &Account, _actx: &AuthContext, _ctx: u64) {}

// PASS
#[authenticator]
public entry fun arg_value(_account: &Account, _val: u8, _actx: &AuthContext, _ctx: &TxContext) {}

// PASS
#[authenticator]
public entry fun arg_mutable_value(
    _account: &Account,
    mut _val: u8,
    _actx: &AuthContext,
    _ctx: &TxContext,
) {}

// FAIL Invalid 'entry' parameter type
//#[authenticator]
//public entry fun with_signer(
//    _account: &Account,
//    _s: signer,
//    _actx: &AuthContext,
//    _ctx: &TxContext,
//) {}
