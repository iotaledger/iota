// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// The signature has `r + n >= 2^256` and a recovery id that asks for the x-coordinate `r + n`.
// Recovery fails.

//# init --addresses test=0x0 --accounts A

//# publish

module test::m;

use iota::ecdsa_r1;

public fun recover() {
    let signature =
        x"00000000ffffffff00000000000000004319055258e8617b0c46353d039cdab4000000000000000000000000000000000000000000000000000000000000000102";
    ecdsa_r1::secp256r1_ecrecover(&signature, &b"message", 1);
}

//# run test::m::recover --sender A
