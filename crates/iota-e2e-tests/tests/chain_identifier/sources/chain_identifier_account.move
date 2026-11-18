// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Reads the chain identifier in a transaction and in an authenticator function.
module chain_identifier::chain_identifier_account;

use iota::account;
use iota::authenticator_function::AuthenticatorFunctionRefV1;

#[error(code = 0)]
const EUnexpectedChainIdentifier: vector<u8> = b"The chain identifier differs from the expected one.";

/// An account whose authenticator accepts a transaction that names the chain identifier.
public struct ChainIdentifierAccount has key {
    id: UID,
}

/// Create a shared `ChainIdentifierAccount` authenticated by `authenticator`.
public fun create(
    authenticator: AuthenticatorFunctionRefV1<ChainIdentifierAccount>,
    ctx: &mut TxContext,
) {
    account::create_account_v1(ChainIdentifierAccount { id: object::new(ctx) }, authenticator);
}

/// Abort unless the chain identifier read from `ctx` is `expected`.
public fun assert_chain_identifier(expected: vector<u8>, ctx: &TxContext) {
    assert!(ctx.chain_identifier() == expected, EUnexpectedChainIdentifier);
}

/// Accept the transaction if the chain identifier read from `ctx` is `expected`.
#[authenticator]
public fun authenticate(
    _account: &ChainIdentifierAccount,
    expected: vector<u8>,
    _: &AuthContext,
    ctx: &TxContext,
) {
    assert_chain_identifier(expected, ctx);
}
