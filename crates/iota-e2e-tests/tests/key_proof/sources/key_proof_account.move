// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// An authenticator for a `SmartAccount` whose authenticator is not the built-in one.
module key_proof::key_proof_account;

use iota::smart_account::SmartAccount;

/// Accepts every transaction of the account.
#[authenticator]
public fun authenticate(_: &SmartAccount, _: &AuthContext, _: &TxContext) {}
