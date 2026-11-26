// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// An authenticator of `AbstractAccount` that reads the transaction nonce. It
/// lives outside the `abstract_account` package, which some tests publish at a
/// protocol version whose framework has no `tx_context::tx_nonce`.
module abstract_account_tx_nonce::abstract_account_tx_nonce;

use abstract_account::abstract_account::AbstractAccount;
use abstract_account::abstract_account_keyed::borrow_public_key;
use abstract_account::basic_keyed_aa;

/// Ed25519 signature authenticator that reads the transaction nonce first, so
/// it aborts wherever the nonce is missing.
#[authenticator]
public fun authenticate_ed25519_reading_tx_nonce(
    account: &AbstractAccount,
    signature: vector<u8>,
    actx: &AuthContext,
    ctx: &TxContext,
) {
    ctx.tx_nonce();
    basic_keyed_aa::authenticate_ed25519(
        &signature,
        borrow_public_key(account),
        actx,
        ctx,
    );
}
