// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# publish
module 0x0::signature;

public struct Account has key {
    id: UID,
}

// FAIL Invalid parameter type
#[authenticator]
public fun with_signer(_account: &Account, _s: signer, _actx: &AuthContext, _ctx: &TxContext) {}
