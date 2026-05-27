// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::str::FromStr;

use async_graphql::{
    connection::{Connection, CursorType, Edge},
    *,
};
use diesel::{
    OptionalExtension, QueryableByName,
    sql_types::{BigInt as SqlBigInt, Nullable, Text},
};
use iota_indexer::types::OwnerType;
use iota_types::base_types::TypeTag;
use serde::{Deserialize, Serialize};

use crate::{
    consistency::Checkpointed,
    data::{Db, DbConnection, QueryExecutor},
    error::Error,
    filter, query,
    raw_query::RawQuery,
    types::{
        available_range::AvailableRange,
        big_int::BigInt,
        cursor::{self, Page, RawPaginated, ScanLimited, Target},
        iota_address::IotaAddress,
        move_type::MoveType,
        uint53::UInt53,
    },
};

/// The total balance for a particular coin type.
#[derive(Clone, Debug, SimpleObject)]
pub(crate) struct Balance {
    /// Coin type for the balance, such as 0x2::iota::IOTA
    pub(crate) coin_type: MoveType,
    /// How many coins of this type constitute the balance
    pub(crate) coin_object_count: Option<UInt53>,
    /// Total balance across all coin objects of the coin type
    pub(crate) total_balance: Option<BigInt>,
}

/// Representation of a row of balance information from the DB. We read the
/// balance as a `String` to deal with the large (bigger than 2^63 - 1)
/// balances.
#[derive(QueryableByName)]
pub struct StoredBalance {
    #[diesel(sql_type = Nullable<Text>)]
    pub balance: Option<String>,
    #[diesel(sql_type = Nullable<SqlBigInt>)]
    pub count: Option<i64>,
    #[diesel(sql_type = Text)]
    pub coin_type: String,
}

pub(crate) type Cursor = cursor::JsonCursor<BalanceCursor>;

/// The inner struct for the `Balance`'s cursor. The `coin_type` is used as the
/// cursor, while the `checkpoint_viewed_at` sets the consistent upper bound for
/// the cursor.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub(crate) struct BalanceCursor {
    #[serde(rename = "t")]
    coin_type: String,
    /// The checkpoint sequence number this was viewed at.
    #[serde(rename = "c")]
    checkpoint_viewed_at: u64,
}

impl Balance {
    /// Query for the balance of coins owned by `address`, of coins with type
    /// `coin_type`. Note that `coin_type` is the type of
    /// `0x2::coin::Coin`'s type parameter, not the full type of the coin
    /// object.
    pub(crate) async fn query(
        db: &Db,
        address: IotaAddress,
        coin_type: TypeTag,
        checkpoint_viewed_at: u64,
    ) -> Result<Option<Balance>, Error> {
        let max_available_range = db.max_available_range;
        let stored: Option<StoredBalance> = db
            .execute_repeatable(move |conn| {
                if !AvailableRange::is_checkpoint_in_backward_history_range(
                    conn,
                    checkpoint_viewed_at,
                    max_available_range,
                )? {
                    return Ok::<_, diesel::result::Error>(None);
                }

                conn.result(move || {
                    balance_query(address, Some(coin_type.clone()), checkpoint_viewed_at)
                        .into_boxed()
                })
                .optional()
            })
            .await?;

        stored.map(Balance::try_from).transpose()
    }

    /// Query the database for a `page` of coin balances. Each balance
    /// represents the total balance for a particular coin type, owned by
    /// `address`.
    pub(crate) async fn paginate(
        db: &Db,
        page: Page<Cursor>,
        address: IotaAddress,
        checkpoint_viewed_at: u64,
    ) -> Result<Connection<String, Balance>, Error> {
        // If cursors are provided, defer to the `checkpoint_viewed_at` in the cursor if
        // they are consistent. Otherwise, use the value from the parameter, or
        // set to None. This is so that paginated queries are consistent with
        // the previous query that created the cursor.
        let cursor_viewed_at = page.validate_cursor_consistency()?;
        let checkpoint_viewed_at = cursor_viewed_at.unwrap_or(checkpoint_viewed_at);

        let max_available_range = db.max_available_range;

        let Some((prev, next, results)) = db
            .execute_repeatable(move |conn| {
                if !AvailableRange::is_checkpoint_in_backward_history_range(
                    conn,
                    checkpoint_viewed_at,
                    max_available_range,
                )? {
                    return Ok::<_, diesel::result::Error>(None);
                }

                let result = page.paginate_raw_query::<StoredBalance>(
                    conn,
                    checkpoint_viewed_at,
                    balance_query(address, None, checkpoint_viewed_at),
                )?;

                Ok(Some(result))
            })
            .await?
        else {
            return Err(Error::Client(
                "Requested data is outside the available range".to_string(),
            ));
        };

        let mut conn = Connection::new(prev, next);
        for stored in results {
            let cursor = stored.cursor(checkpoint_viewed_at).encode_cursor();
            let balance = Balance::try_from(stored)?;
            conn.edges.push(Edge::new(cursor, balance));
        }

        Ok(conn)
    }
}

impl RawPaginated<Cursor> for StoredBalance {
    fn filter_ge(cursor: &Cursor, query: RawQuery) -> RawQuery {
        filter!(query, "coin_type >= {}", cursor.coin_type.clone())
    }

    fn filter_le(cursor: &Cursor, query: RawQuery) -> RawQuery {
        filter!(query, "coin_type <= {}", cursor.coin_type.clone())
    }

    fn order(asc: bool, query: RawQuery) -> RawQuery {
        if asc {
            return query.order_by("coin_type ASC");
        }
        query.order_by("coin_type DESC")
    }
}

impl Target<Cursor> for StoredBalance {
    fn cursor(&self, checkpoint_viewed_at: u64) -> Cursor {
        Cursor::new(BalanceCursor {
            coin_type: self.coin_type.clone(),
            checkpoint_viewed_at,
        })
    }
}

impl Checkpointed for Cursor {
    fn checkpoint_viewed_at(&self) -> u64 {
        self.checkpoint_viewed_at
    }
}

impl ScanLimited for Cursor {}

impl TryFrom<StoredBalance> for Balance {
    type Error = Error;

    fn try_from(s: StoredBalance) -> Result<Self, Error> {
        let StoredBalance {
            balance,
            count,
            coin_type,
        } = s;
        let total_balance = balance
            .map(|b| BigInt::from_str(&b))
            .transpose()
            .map_err(|_| Error::Internal("Failed to read balance.".to_string()))?;

        let coin_object_count = count.map(|c| UInt53::from(c as u64));

        let coin_type = TypeTag::from_str(&coin_type)
            .map_err(|e| Error::Internal(format!("Failed to parse coin type: {e}")))?
            .into();

        Ok(Balance {
            coin_type,
            coin_object_count,
            total_balance,
        })
    }
}

/// Builds the aggregating SQL that totals each coin type's balance for
/// `address` at `checkpoint_viewed_at`, using the backward-diff tables.
///
/// The consistent set of objects at `checkpoint_viewed_at` is the union of two
/// disjoint sources:
///
/// * Source A — `checkpointed_objects` rows for objects that have not changed
///   since `checkpoint_viewed_at` (no `objects_backward_history` entry with
///   `superseded_at_checkpoint > checkpoint_viewed_at`).
/// * Source B — `objects_backward_history` rows for the version of each object
///   current at `checkpoint_viewed_at`: the earliest version whose
///   `superseded_at_checkpoint > checkpoint_viewed_at`. Synthetic rows
///   (`NotYetCreated`, `WrappedOrDeleted`) drop out automatically — they carry
///   NULL `owner_id`/`coin_type`, so the owner+coin filter excludes them.
///
/// Each object can match at most one source: A only returns objects that
/// did not change after `checkpoint_viewed_at`, while B only returns objects
/// that did. So `UNION ALL` already gives one row per `object_id` — no
/// dedup needed before aggregation.
fn balance_query(
    address: IotaAddress,
    coin_type: Option<TypeTag>,
    checkpoint_viewed_at: u64,
) -> RawQuery {
    let checkpoint_viewed_at = checkpoint_viewed_at as i64;

    let checkpointed = filter(
        query!("SELECT object_id, coin_balance, coin_type FROM checkpointed_objects"),
        address,
        coin_type.clone(),
    );
    let changed = query!(format!(
        "SELECT DISTINCT object_id FROM objects_backward_history \
         WHERE superseded_at_checkpoint > {checkpoint_viewed_at}"
    ));
    let source_a = filter!(
        query!(
            r#"SELECT candidates.object_id, candidates.coin_balance, candidates.coin_type
               FROM ({}) candidates
               LEFT JOIN ({}) changed ON candidates.object_id = changed.object_id"#,
            checkpointed,
            changed
        ),
        "changed.object_id IS NULL"
    );

    let mut history = filter(
        query!(
            "SELECT object_id, object_version, coin_balance, coin_type \
             FROM objects_backward_history"
        ),
        address,
        coin_type,
    );
    // NotYetCreated and WrappedOrDeleted rows are already excluded above by
    // `filter`'s `coin_type IS NOT NULL` / `owner_id = ...` clauses, since
    // `from_empty` leaves those columns NULL.
    history = filter!(
        history,
        format!("superseded_at_checkpoint > {checkpoint_viewed_at}")
    );
    let oldest = query!(format!(
        "SELECT object_id, MIN(object_version) AS min_version \
         FROM objects_backward_history \
         WHERE superseded_at_checkpoint > {checkpoint_viewed_at} \
         GROUP BY object_id"
    ));
    let source_b = query!(
        r#"SELECT candidates.object_id, candidates.coin_balance, candidates.coin_type
           FROM ({}) candidates
           JOIN ({}) oldest ON candidates.object_id = oldest.object_id
               AND candidates.object_version = oldest.min_version"#,
        history,
        oldest
    );

    query!(
        r#"SELECT
            CAST(SUM(coin_balance) AS TEXT) as balance,
            COUNT(*) as count,
            coin_type
        FROM (({}) UNION ALL ({})) candidates"#,
        source_a,
        source_b
    )
    .group_by("coin_type")
}

/// Applies the filtering criteria for balances to the input `RawQuery` and
/// returns a new `RawQuery`.
fn filter(mut query: RawQuery, owner: IotaAddress, coin_type: Option<TypeTag>) -> RawQuery {
    query = filter!(query, "coin_type IS NOT NULL");

    query = filter!(
        query,
        format!(
            "owner_id = '\\x{}'::bytea AND owner_type = {}",
            hex::encode(owner.into_vec()),
            OwnerType::Address as i16
        )
    );

    if let Some(coin_type) = coin_type {
        query = filter!(
            query,
            "coin_type = {}",
            coin_type.to_canonical_string(/* with_prefix */ true)
        );
    };

    query
}
