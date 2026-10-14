// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// A Move account whose authenticator either always passes or always aborts.
/// The two aborting authenticators abort with different codes, so a test can
/// tell which one failed.
module move_authenticator::test_account;

use iota::account;
use iota::auth_context::AuthContext;
use iota::authenticator_function::AuthenticatorFunctionRefV1;

public struct TestAccount has key {
    id: UID,
}

/// Creates a shared account authenticated by `authenticator`.
public fun create(authenticator: AuthenticatorFunctionRefV1<TestAccount>, ctx: &mut TxContext) {
    account::create_account_v1(TestAccount { id: object::new(ctx) }, authenticator);
}

#[authenticator]
public fun authenticate_pass(_: &TestAccount, _: &AuthContext, _: &TxContext) {}

#[authenticator]
public fun authenticate_fail(_: &TestAccount, _: &AuthContext, _: &TxContext) {
    abort 1
}

#[authenticator]
public fun authenticate_fail_other(_: &TestAccount, _: &AuthContext, _: &TxContext) {
    abort 2
}
