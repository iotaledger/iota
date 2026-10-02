// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// A custom authenticator for framework `SmartAccount`s, so that a test can
/// rotate an account away from the built-in authenticators. It accepts every
/// transaction; the tests never send one through it.
module custom_authenticator::custom_authenticator;

use iota::smart_account::SmartAccount;

#[authenticator]
public fun authenticate(
    _account: &SmartAccount,
    _signature: vector<u8>,
    _actx: &AuthContext,
    _ctx: &TxContext,
) {}
