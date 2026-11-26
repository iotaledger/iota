// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Reads the nonce of the transaction in a transaction and in an authenticator function.
module tx_nonce::tx_nonce_account;

use iota::account;
use iota::authenticator_function::AuthenticatorFunctionRefV1;

#[error(code = 0)]
const EUnexpectedTxNonce: vector<u8> = b"The nonce of the transaction differs from the expected one.";

/// An account authenticated by `authenticate` or `authenticate_without_tx_nonce`.
public struct TxNonceAccount has key {
    id: UID,
}

/// Create a shared `TxNonceAccount` authenticated by `authenticator`.
public fun create(
    authenticator: AuthenticatorFunctionRefV1<TxNonceAccount>,
    ctx: &mut TxContext,
) {
    account::create_account_v1(TxNonceAccount { id: object::new(ctx) }, authenticator);
}

/// Abort unless the nonce read from `ctx` is `expected`.
public fun assert_tx_nonce(expected: vector<u8>, ctx: &TxContext) {
    assert!(ctx.tx_nonce() == expected, EUnexpectedTxNonce);
}

/// Accept the transaction only if the nonce read from `ctx` is `expected`.
#[authenticator]
public fun authenticate(
    _account: &TxNonceAccount,
    expected: vector<u8>,
    _: &AuthContext,
    ctx: &TxContext,
) {
    assert_tx_nonce(expected, ctx);
}

/// Accept every transaction, without reading the nonce.
#[authenticator]
public fun authenticate_without_tx_nonce(
    _account: &TxNonceAccount,
    _: &AuthContext,
    _: &TxContext,
) {}
