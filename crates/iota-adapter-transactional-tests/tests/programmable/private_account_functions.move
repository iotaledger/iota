// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// tests calling private account functions

//# init --addresses test=0x0 --accounts A

//# publish
module test::m1;

public struct Account has key { id: UID }

public fun account(ctx: &mut TxContext): Account { Account { id: object::new(ctx) } }

//# programmable --inputs @test "module" "function"
//> 0: test::m1::account();
//> 1: iota::account::create_auth_info_v1_for_testing(Input(0), Input(1), Input(2));
//> iota::account::create_shared_account_v1<test::m1::Account>(Result(0), Result(1));

//# programmable --inputs @test "module" "function"
//> 0: test::m1::account();
//> 1: iota::account::create_auth_info_v1_for_testing(Input(0), Input(1), Input(2));
//> iota::account::create_immutable_account_v1<test::m1::Account>(Result(0));

//# programmable --inputs @test "module" "function"
//> 0: test::m1::account();
//> 1: iota::account::create_auth_info_v1_for_testing(Input(0), Input(1), Input(2));
//> iota::account::rotate_auth_info_v1<test::m1::Account>(Result(0));
