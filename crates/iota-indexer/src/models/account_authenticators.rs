// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use diesel::prelude::*;

use crate::schema::account_authenticators;

/// The current authenticator kind of a framework `SmartAccount`, with or
/// without a built-in key.
///
/// Written from `iota::account`'s creation and authenticator-rotation events.
/// The values of `kind` are those of
/// [`crate::account_key_events::AuthenticatorKind`].
#[derive(Debug, Clone, PartialEq, Eq, Queryable, Insertable, Selectable)]
#[diesel(table_name = account_authenticators)]
pub struct StoredAccountAuthenticator {
    pub account_id: Vec<u8>,
    pub kind: i16,
    pub last_change_tx_sequence_number: i64,
    pub last_change_epoch: i64,
}
