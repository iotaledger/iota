// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeMap;

use async_graphql::{
    connection::{Connection, CursorType, Edge},
    *,
};
use diesel::{ExpressionMethods, QueryDsl, SelectableHelper};
use iota_indexer::{
    models::{objects::StoredObject, smart_account_keys::StoredSmartAccountKey},
    schema::{objects, smart_account_keys},
};
use serde::{Deserialize, Serialize};

use crate::{
    consistency::{Checkpointed, LiveCursor, UNAVAILABLE_CHECKPOINT_SEQUENCE_NUMBER},
    data::{self, Db, DbConnection, QueryExecutor},
    error::Error,
    types::{
        base64::Base64,
        cursor::{self, Page, Paginated, Target},
        move_object::MoveObject,
        object::Object,
    },
};

/// Selects the `SmartAccount`s by the public key attached to them.
#[derive(InputObject, Debug, Clone)]
pub(crate) struct SmartAccountFilter {
    /// The hash of the public key: blake2b256(scheme flag || raw public key
    /// bytes).
    pub key_id: Base64,
}

pub(crate) type Cursor = cursor::BcsCursor<SmartAccountCursor>;

/// The cursor returned for each `SmartAccount` in a page.
/// It contains no checkpoint, since the page is always a live view.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub(crate) struct SmartAccountCursor {
    #[serde(rename = "a")]
    account_id: Vec<u8>,
}

type Query<ST, GB> = data::Query<ST, smart_account_keys::table, GB>;

/// Query the database for a `page` of the `SmartAccount`s selected by
/// `filter`, ordered by their id.
///
/// The page shows the live state: the keys and the accounts as the indexer
/// last wrote them. The nested fields of the accounts are read as of
/// `checkpoint_viewed_at`, the latest checkpoint indexed when the request is
/// served. Pages are not a snapshot, every page serves the current live
/// data.
pub(crate) async fn paginate(
    ctx: &Context<'_>,
    mut page: Page<Cursor>,
    filter: SmartAccountFilter,
    checkpoint_viewed_at: u64,
) -> Result<Connection<String, MoveObject>, Error> {
    let SmartAccountFilter {
        key_id: Base64(key_id),
    } = filter;
    if key_id.len() != 32 {
        return Err(Error::Client(format!(
            "keyId must be 32 bytes, got {}",
            key_id.len()
        )));
    }

    page.set_live_view(checkpoint_viewed_at);
    let db: &Db = ctx.data_unchecked();
    let (prev, next, keys, accounts) = db
        .execute_repeatable(move |conn| {
            let (prev, next, keys) = page.paginate_query::<StoredSmartAccountKey, _, _, _>(
                conn,
                checkpoint_viewed_at,
                move || {
                    smart_account_keys::table
                        .filter(smart_account_keys::key_id.eq(key_id.clone()))
                        .into_boxed()
                },
            )?;
            let keys: Vec<StoredSmartAccountKey> = keys.collect();
            if keys.is_empty() {
                return Ok::<_, diesel::result::Error>((prev, next, keys, vec![]));
            }
            let account_ids: Vec<Vec<u8>> = keys.iter().map(|key| key.account_id.clone()).collect();
            let accounts: Vec<StoredObject> = conn.results(move || {
                objects::table
                    .filter(objects::object_id.eq_any(account_ids.clone()))
                    .select(StoredObject::as_select())
            })?;
            Ok((prev, next, keys, accounts))
        })
        .await?;

    let mut accounts: BTreeMap<Vec<u8>, StoredObject> = accounts
        .into_iter()
        .map(|account| (account.object_id.clone(), account))
        .collect();

    let mut connection = Connection::new(prev, next);
    for key in keys {
        // The account and key row are not always written atomically.
        // If for that reason account object is missing, we skip.
        let Some(stored) = accounts.remove(&key.account_id) else {
            continue;
        };
        let object = Object::try_from_stored_object(stored, checkpoint_viewed_at, None)?;
        let account = MoveObject::try_from(&object)
            .map_err(|_| Error::Internal(format!("{} is not a Move object", object.address)))?;
        let cursor = key.cursor(checkpoint_viewed_at).encode_cursor();
        connection.edges.push(Edge::new(cursor, account));
    }
    Ok(connection)
}

impl Checkpointed for Cursor {
    /// The cursor carries no checkpoint, the page is always a live view.
    fn checkpoint_viewed_at(&self) -> u64 {
        UNAVAILABLE_CHECKPOINT_SEQUENCE_NUMBER
    }
}

impl LiveCursor for Cursor {
    /// Nothing to set, the cursor carries no checkpoint.
    fn set_checkpoint_viewed_at(&mut self, _checkpoint_viewed_at: u64) {}
}

impl Target<Cursor> for StoredSmartAccountKey {
    fn cursor(&self, _checkpoint_viewed_at: u64) -> Cursor {
        Cursor::new(SmartAccountCursor {
            account_id: self.account_id.clone(),
        })
    }
}

impl Paginated<Cursor> for StoredSmartAccountKey {
    type Source = smart_account_keys::table;

    fn filter_ge<ST, GB>(cursor: &Cursor, query: Query<ST, GB>) -> Query<ST, GB> {
        query.filter(smart_account_keys::account_id.ge(cursor.account_id.clone()))
    }

    fn filter_le<ST, GB>(cursor: &Cursor, query: Query<ST, GB>) -> Query<ST, GB> {
        query.filter(smart_account_keys::account_id.le(cursor.account_id.clone()))
    }

    fn order<ST, GB>(asc: bool, query: Query<ST, GB>) -> Query<ST, GB> {
        if asc {
            query.order_by(smart_account_keys::account_id.asc())
        } else {
            query.order_by(smart_account_keys::account_id.desc())
        }
    }
}
