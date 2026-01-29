// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# init --addresses Test=0x0

//# publish

module Test::M;

public fun authenticate(_: &AuthContext, _: &TxContext) {}

public fun authenticate_with_mut_auth_context(_: &mut AuthContext, _: &TxContext) {}

public fun authenticate_with_auth_context_value(_: AuthContext, _: &TxContext) {}

public fun authenticate_with_miss_placed_auth_context(_: &AuthContext, _: u64, _: &TxContext) {}

public fun authenticate_with_only_auth_context(_: &AuthContext) {}

// using `iota::auth_context::AuthContext` is not allowed in this execution mode
//# run Test::M::authenticate

// using a mutable reference to `iota::auth_context::AuthContext` is not allowed
//# run Test::M::authenticate_with_mut_auth_context

// using `iota::auth_context::AuthContext` by value is not allowed
//# run Test::M::authenticate_with_auth_context_value

// `iota::auth_context::AuthContext` must be the second last argument
//# run Test::M::authenticate_with_miss_placed_auth_context

// `iota::auth_context::AuthContext` must be the second last argument
//# run Test::M::authenticate_with_only_auth_context
