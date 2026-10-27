// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// A Move account whose authenticator either always passes or always aborts.
/// The two aborting authenticators abort with different codes, so a test can
/// tell which one failed. The object can also be shared first and made an
/// account later, and receive objects until then.
module move_authenticator::test_account;

use iota::account;
use iota::auth_context::AuthContext;
use iota::authenticator_function::AuthenticatorFunctionRefV1;
use iota::transfer::Receiving;

public struct TestAccount has key {
    id: UID,
}

/// Creates a shared account authenticated by `authenticator`.
public fun create(authenticator: AuthenticatorFunctionRefV1<TestAccount>, ctx: &mut TxContext) {
    account::create_account_v1(TestAccount { id: object::new(ctx) }, authenticator);
}

/// Shares an object that is not an account yet. `make_account` makes it one.
public fun create_delayed(ctx: &mut TxContext) {
    transfer::share_object(TestAccount { id: object::new(ctx) });
}

/// Makes `account`, shared by `create_delayed`, an account authenticated by
/// `authenticator`.
public fun make_account(
    account: TestAccount,
    authenticator: AuthenticatorFunctionRefV1<TestAccount>,
) {
    account::create_account_v1(account, authenticator);
}

/// Receives `object` through `account` and sends it back to the account's
/// address, which gives it a new version. Aborts once `account` is an account.
public fun send_back<T: key + store>(account: &mut TestAccount, object: Receiving<T>) {
    let object = transfer::public_receive(&mut account.id, object);
    transfer::public_transfer(object, account.id.to_address());
}

/// Deletes `account`.
public fun delete(account: TestAccount) {
    let TestAccount { id } = account;
    id.delete();
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
