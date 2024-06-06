// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# init --accounts A B --addresses test=0x0

//# publish --sender A
module test::random {
    use iota::random::Random;

    public fun use_random(_random: &Random) {}
}

// bad tx - use_random twice
//# programmable --sender A --inputs immshared(8) @A
//> test::random::use_random(Input(0));
//> test::random::use_random(Input(0));
