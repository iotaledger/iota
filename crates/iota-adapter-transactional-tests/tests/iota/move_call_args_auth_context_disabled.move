// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# init --addresses Test=0x0 --move-auth false

//# publish

module Test::M;

public fun authenticate(_: &AuthContext, _: &TxContext) {}

// using `iota::auth_context::AuthContext` is not allowed by the protocol config
//# run Test::M::authenticate
