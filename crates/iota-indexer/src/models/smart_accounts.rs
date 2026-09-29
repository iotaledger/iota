// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use diesel::prelude::*;

use crate::schema::smart_accounts;

/// A framework `SmartAccount`, with or without a built-in key.
///
/// Written only from `SmartAccountCreated`. An address with a key link but no
/// row here is some other object a key was attached to, not a `SmartAccount`.
#[derive(Debug, Clone, PartialEq, Eq, Queryable, Insertable, Selectable)]
#[diesel(table_name = smart_accounts)]
pub struct StoredSmartAccount {
    pub account_id: Vec<u8>,
    /// Whether the account was frozen at creation.
    pub immutable: bool,
    pub created_tx_sequence_number: i64,
    pub created_epoch: i64,
}
