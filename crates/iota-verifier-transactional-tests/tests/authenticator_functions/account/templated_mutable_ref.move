// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# publish
module 0x0::account;

#[allow(unused_field)]
public struct Account<T: store> has key {
    id: UID,
    wrapped: T,
}

// FAIL
#[authenticator]
public fun templated_mutable_ref<T: store>(
    _account: &mut Account<T>,
    _actx: &AuthContext,
    _ctx: &TxContext,
) {}
