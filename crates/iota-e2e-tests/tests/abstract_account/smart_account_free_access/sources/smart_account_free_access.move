// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// A custom authenticator for `iota::smart_account::SmartAccount`.
///
/// This lives in its own package because `iota::smart_account` only exists in
/// the framework from protocol version 42.
module smart_account_free_access::smart_account_free_access;

use iota::smart_account::SmartAccount;

/// Free access, do nothing.
#[authenticator]
public fun authenticate(_: &SmartAccount, _: &AuthContext, _: &TxContext) {}
