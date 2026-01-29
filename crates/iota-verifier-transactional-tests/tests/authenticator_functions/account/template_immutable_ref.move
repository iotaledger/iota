// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# publish
module 0x0::account;

// FAIL
#[authenticator]
public fun template_immutable_ref<T: key>(_account: &T, _actx: &AuthContext, _ctx: &TxContext) {}
