// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// tests various calls of non-primitive argument usage

//# init --addresses test=0x0 --accounts A

//# publish
module test::m1 {
    public struct Potato {}

    public fun potato(): Potato { Potato {} }
    public fun otatop(tater: Potato) { let Potato {} = tater; }

    public fun pass(tater: Potato): Potato { tater }
    public fun imm(_: &Potato) {}
    public fun mut_(_: &mut Potato) {}

}

//# programmable
//> 0: test::m1::potato();
//> 1: test::m1::pass(Result(0));
//> test::m1::imm(Result(1));
//> test::m1::mut_(Result(1));
//> test::m1::otatop(Result(1));
