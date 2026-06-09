// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# publish
module 0x0::option;

public struct Account has key {
    id: UID,
}

public struct NonObject has copy, drop, store {}

// FAIL Invalid parameter type
#[authenticator]
public fun non_object_mutable_ref(
    _account: &Account,
    _arg: &mut Option<NonObject>,
    _actx: &AuthContext,
    _ctx: &TxContext,
) {}
