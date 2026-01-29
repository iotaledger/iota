// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// tests calling private account functions

//# init --addresses test=0x0 --accounts A

//# publish
module test::account;

public struct Account has key { id: UID }

public fun create(ctx: &mut TxContext): Account { Account { id: object::new(ctx) } }

#[authenticator]
public fun authenticate(_: &Account, _auth_ctx: &AuthContext, _ctx: &TxContext) {}

//# programmable --inputs object(1,1) "account" "authenticate"
//> 0: test::account::create();
//> 1: iota::authenticator_function::create_auth_function_ref_v1<test::account::Account>(Input(0), Input(1), Input(2));
//> 2: iota::account::create_account_v1<test::account::Account>(Result(0), Result(1));

//# programmable --inputs object(1,1) "account" "authenticate"
//> 0: test::account::create();
//> 1: iota::authenticator_function::create_auth_function_ref_v1<test::account::Account>(Input(0), Input(1), Input(2));
//> 2: iota::account::create_immutable_account_v1<test::account::Account>(Result(0), Result(1));

//# programmable --inputs object(1,1) "account" "authenticate"
//> 0: test::account::create();
//> 1: iota::authenticator_function::create_auth_function_ref_v1<test::account::Account>(Input(0), Input(1), Input(2));
//> 2: iota::account::rotate_auth_function_ref_v1<test::account::Account>(Result(0), Result(1));
