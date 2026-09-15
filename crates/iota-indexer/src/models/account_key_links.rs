// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use diesel::prelude::*;

use crate::{
    account_key_events::{AccountKeyLinkOp, LinkOpKind},
    schema::account_key_links,
};

/// The key currently controls the account.
pub const LINK_STATUS_ACTIVE: i16 = 0;
/// The key used to control the account and was rotated away or detached. Kept
/// as a tombstone so a wallet can still recognize an account it once held.
pub const LINK_STATUS_UNLINKED: i16 = 1;

/// One `(key_id, account_id)` pair with its latest link state — a row of the
/// materialized fold of the discoverability event stream, not a history entry.
#[derive(Debug, Clone, PartialEq, Eq, Queryable, Insertable, Selectable)]
#[diesel(table_name = account_key_links)]
pub struct StoredAccountKeyLink {
    pub key_id: Vec<u8>,
    pub account_id: Vec<u8>,
    pub scheme: i16,
    pub source: i16,
    pub status: i16,
    pub last_change_tx_sequence_number: i64,
    pub last_change_epoch: i64,
}

impl From<&AccountKeyLinkOp> for StoredAccountKeyLink {
    fn from(op: &AccountKeyLinkOp) -> Self {
        Self {
            key_id: op.key_id.to_vec(),
            account_id: op.account_id.to_vec(),
            scheme: op.scheme as i16,
            source: op.source as i16,
            status: match op.kind {
                LinkOpKind::Link => LINK_STATUS_ACTIVE,
                LinkOpKind::Unlink => LINK_STATUS_UNLINKED,
            },
            last_change_tx_sequence_number: op.tx_sequence_number,
            last_change_epoch: op.epoch,
        }
    }
}
