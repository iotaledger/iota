// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// A Move account whose authenticator takes a type parameter that none of its
/// arguments uses, which the authenticator verifier accepts.
module move_authenticator_generic::generic_account;

use iota::account;
use iota::auth_context::AuthContext;
use iota::authenticator_function::AuthenticatorFunctionRefV1;

public struct GenericAccount has key {
    id: UID,
}

/// Creates a shared account authenticated by `authenticator`.
public fun create(
    authenticator: AuthenticatorFunctionRefV1<GenericAccount>,
    ctx: &mut TxContext,
) {
    account::create_account_v1(GenericAccount { id: object::new(ctx) }, authenticator);
}

#[authenticator]
public fun authenticate<T>(_: &GenericAccount, _: &AuthContext, _: &TxContext) {}
