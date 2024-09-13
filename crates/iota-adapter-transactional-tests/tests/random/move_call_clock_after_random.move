// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# init --accounts A B --addresses test=0x0

//# publish --sender A
module test::random {
    use iota::clock::Clock;
    use iota::random::Random;

    public fun use_clock(_clock: &Clock) {}
    public fun use_random(_random: &Random) {}
}

// bad tx - use_random, use_clock
//# programmable --sender A --inputs immshared(8) immshared(6) @A
//> test::random::use_random(Input(0));
//> test::random::use_clock(Input(1))
