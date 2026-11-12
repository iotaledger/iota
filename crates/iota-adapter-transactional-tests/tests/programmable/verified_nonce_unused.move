// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// a VerifiedNonce has no abilities, so a transaction that takes one and leaves it unused fails

//# init --addresses test=0x0 --accounts A

//# publish
module test::m1 {
    public struct Check {}
}

//# programmable --sender A
//> 0: iota::verified_nonce::take_from_tx_context<test::m1::Check>();
