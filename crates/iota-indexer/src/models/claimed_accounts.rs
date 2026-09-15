// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use diesel::prelude::*;

use crate::schema::claimed_accounts;

/// An account created by a `ClaimAccount` transaction.
///
/// Written only from `SmartAccountClaimed`. Absence of a row means the account
/// was not claimed — which is how an account someone else created with your key
/// is told apart from one you claimed yourself.
#[derive(Debug, Clone, PartialEq, Eq, Queryable, Insertable, Selectable)]
#[diesel(table_name = claimed_accounts)]
pub struct StoredClaimedAccount {
    pub account_id: Vec<u8>,
    pub key_id: Vec<u8>,
    /// Whether the account was frozen at creation. An immutable account's key
    /// can never be rotated or detached, so its links are permanent.
    pub immutable: bool,
    pub claim_tx_sequence_number: i64,
    pub claim_epoch: i64,
}
