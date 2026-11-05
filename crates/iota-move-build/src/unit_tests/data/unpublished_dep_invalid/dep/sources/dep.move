// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// An unpublished dependency whose authenticator function lacks the
/// `&AuthContext` argument, which the verifier of a bundled publish must
/// reject.
module dep::dep;

public struct Account has key {
    id: UID,
}

#[authenticator]
public fun authenticate(_account: &Account, _ctx: &TxContext) {}
