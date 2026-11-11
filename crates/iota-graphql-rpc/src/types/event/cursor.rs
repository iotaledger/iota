// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use diesel::{
    BoolExpressionMethods, ExpressionMethods, QueryDsl,
    backend::Backend,
    deserialize::{self, FromSql, QueryableByName},
    row::NamedRow,
};
use iota_indexer::{models::events::StoredEvent, schema::events};
use serde::{Deserialize, Serialize};

use crate::{
    consistency::Checkpointed,
    filter,
    raw_query::RawQuery,
    types::{
        cursor::{self, Paginated, RawPaginated, ScanLimited, Target},
        event::Query,
        uint53::UInt53,
    },
};

/// Contents of an Event's cursor.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
pub(crate) struct EventKey {
    /// Transaction Sequence Number
    pub tx: UInt53,

    /// Event Sequence Number
    pub e: UInt53,

    /// The checkpoint sequence number this was viewed at.
    #[serde(rename = "c")]
    pub checkpoint_viewed_at: UInt53,
}

pub(crate) type Cursor = cursor::JsonCursor<EventKey>;

/// Results from raw queries in Diesel can only be deserialized into structs
/// that implement `QueryableByName`. This struct is used to represent a row of
/// `tx_sequence_number` and `event_sequence_number` returned from subqueries
/// against event lookup tables.
#[derive(Clone, Debug)]
pub struct EvLookup {
    pub tx: i64,
    pub ev: i64,
}

impl Paginated<Cursor> for StoredEvent {
    type Source = events::table;

    fn filter_ge<ST, GB>(cursor: &Cursor, query: Query<ST, GB>) -> Query<ST, GB> {
        use events::dsl::{event_sequence_number as event, tx_sequence_number as tx};
        query.filter(
            tx.gt(i64::from(cursor.tx)).or(tx
                .eq(i64::from(cursor.tx))
                .and(event.ge(i64::from(cursor.e)))),
        )
    }

    fn filter_le<ST, GB>(cursor: &Cursor, query: Query<ST, GB>) -> Query<ST, GB> {
        use events::dsl::{event_sequence_number as event, tx_sequence_number as tx};
        query.filter(
            tx.lt(i64::from(cursor.tx)).or(tx
                .eq(i64::from(cursor.tx))
                .and(event.le(i64::from(cursor.e)))),
        )
    }

    fn order<ST, GB>(asc: bool, query: Query<ST, GB>) -> Query<ST, GB> {
        use events::dsl;
        if asc {
            query
                .order_by(dsl::tx_sequence_number.asc())
                .then_order_by(dsl::event_sequence_number.asc())
        } else {
            query
                .order_by(dsl::tx_sequence_number.desc())
                .then_order_by(dsl::event_sequence_number.desc())
        }
    }
}

impl RawPaginated<Cursor> for StoredEvent {
    fn filter_ge(cursor: &Cursor, query: RawQuery) -> RawQuery {
        filter!(
            query,
            format!(
                "ROW(tx_sequence_number, event_sequence_number) >= ({}, {})",
                cursor.tx, cursor.e
            )
        )
    }

    fn filter_le(cursor: &Cursor, query: RawQuery) -> RawQuery {
        filter!(
            query,
            format!(
                "ROW(tx_sequence_number, event_sequence_number) <= ({}, {})",
                cursor.tx, cursor.e
            )
        )
    }

    fn order(asc: bool, query: RawQuery) -> RawQuery {
        if asc {
            query.order_by("tx_sequence_number ASC, event_sequence_number ASC")
        } else {
            query.order_by("tx_sequence_number DESC, event_sequence_number DESC")
        }
    }
}

impl Target<Cursor> for StoredEvent {
    fn cursor(&self, checkpoint_viewed_at: u64) -> Cursor {
        Cursor::new(EventKey {
            tx: UInt53::new_unchecked(self.tx_sequence_number as u64),
            e: UInt53::new_unchecked(self.event_sequence_number as u64),
            checkpoint_viewed_at: UInt53::new_unchecked(checkpoint_viewed_at),
        })
    }
}

impl Checkpointed for Cursor {
    fn checkpoint_viewed_at(&self) -> u64 {
        u64::from(self.checkpoint_viewed_at)
    }
}

impl ScanLimited for Cursor {}

impl Target<Cursor> for EvLookup {
    fn cursor(&self, checkpoint_viewed_at: u64) -> Cursor {
        Cursor::new(EventKey {
            tx: UInt53::new_unchecked(self.tx as u64),
            e: UInt53::new_unchecked(self.ev as u64),
            checkpoint_viewed_at: UInt53::new_unchecked(checkpoint_viewed_at),
        })
    }
}

impl RawPaginated<Cursor> for EvLookup {
    fn filter_ge(cursor: &Cursor, query: RawQuery) -> RawQuery {
        filter!(
            query,
            format!(
                "ROW(tx_sequence_number, event_sequence_number) >= ({}, {})",
                cursor.tx, cursor.e
            )
        )
    }

    fn filter_le(cursor: &Cursor, query: RawQuery) -> RawQuery {
        filter!(
            query,
            format!(
                "ROW(tx_sequence_number, event_sequence_number) <= ({}, {})",
                cursor.tx, cursor.e
            )
        )
    }

    fn order(asc: bool, query: RawQuery) -> RawQuery {
        if asc {
            query.order_by("tx_sequence_number ASC, event_sequence_number ASC")
        } else {
            query.order_by("tx_sequence_number DESC, event_sequence_number DESC")
        }
    }
}

/// `sql_query` raw queries require `QueryableByName`. The default
/// implementation looks for a table based on the struct name, and it also
/// expects the struct's fields to reflect the table's columns. We can override
/// this behavior by implementing `QueryableByName` for our struct. For
/// `EvLookup`, its fields are derived from the common `tx_sequence_number` and
/// `event_sequence_number` columns for all events-related tables.
impl<DB> QueryableByName<DB> for EvLookup
where
    DB: Backend,
    i64: FromSql<diesel::sql_types::BigInt, DB>,
{
    fn build<'a>(row: &impl NamedRow<'a, DB>) -> deserialize::Result<Self> {
        let tx = NamedRow::get::<diesel::sql_types::BigInt, _>(row, "tx_sequence_number")?;
        let ev = NamedRow::get::<diesel::sql_types::BigInt, _>(row, "event_sequence_number")?;

        Ok(Self { tx, ev })
    }
}
