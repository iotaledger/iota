// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# publish
module 0x0::account;

use iota::auth_context::AuthContext;

// FAIL
#[authenticator]
public fun template_mutable_ref<T: key>(_account: &mut T, _actx: &AuthContext, _ctx: &TxContext) {}
