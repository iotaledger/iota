// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# publish
module 0x0::vector;

public struct Account has key {
    id: UID,
}

public struct NonObjectTemplated<T: copy + drop + store> has copy, drop, store {
    t: T,
}

// FAIL Invalid parameter type
#[authenticator]
public fun templated_non_object_mutable_ref<T: copy + drop + store>(
    _account: &Account,
    _arg: &mut vector<NonObjectTemplated<T>>,
    _actx: &AuthContext,
    _ctx: &TxContext,
) {}
