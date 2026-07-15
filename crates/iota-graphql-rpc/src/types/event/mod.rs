// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::ops::Bound;

use async_graphql::{
    connection::{Connection, CursorType, Edge},
    *,
};
use cursor::EvLookup;
use diesel::{ExpressionMethods, QueryDsl};
use iota_indexer::{
    models::{
        events::StoredEvent,
        transactions::{OptimisticTransaction, StoredTransaction},
    },
    schema::{checkpoints, events},
};
use iota_sdk_types::{Address as NativeAddress, Event as NativeEvent, Identifier, ObjectId};
use iota_types::parse_iota_struct_tag;
use lookups::{add_bounds, select_emit_module, select_event_type, select_sender};

use crate::{
    config::DEFAULT_PAGE_SIZE,
    data::{self, Db, DbConnection, QueryExecutor},
    error::Error,
    query,
    types::{
        address::Address,
        base64::Base64,
        cursor::{Page, Target},
        date_time::DateTime,
        digest::Digest,
        move_module::MoveModule,
        move_value::MoveValue,
        transaction_block::{DigestKey, TransactionBlock},
    },
};

mod cursor;
mod filter;
mod lookups;
pub(crate) use cursor::Cursor;
pub(crate) use filter::EventFilter;

/// An event emitted in a transaction that has been checkpointed.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CheckpointedEventInfo {
    /// The digest of the parent transaction.
    tx_digest: Digest,
    /// The timestamp of the parent transaction.
    timestamp_ms: i64,
}

impl TryFrom<&StoredEvent> for CheckpointedEventInfo {
    type Error = Error;

    fn try_from(value: &StoredEvent) -> Result<Self, Self::Error> {
        let tx_digest = Digest::try_from(value.transaction_digest.as_slice())
            .map_err(|e| Error::Internal(format!("Bad transaction digest on event: {e}")))?;
        Ok(Self {
            tx_digest,
            timestamp_ms: value.timestamp_ms,
        })
    }
}

/// An IOTA node emits one of the following events:
/// Move event
/// Publish event
/// Transfer object event
/// Delete object event
/// New object event
/// Epoch change event
#[derive(Clone, Debug)]
pub(crate) struct Event {
    pub checkpointed_info: Option<CheckpointedEventInfo>,
    pub native: NativeEvent,
    /// The checkpoint sequence number this was viewed at.
    pub checkpoint_viewed_at: u64,
}

type Query<ST, GB> = data::Query<ST, events::table, GB>;

#[Object]
impl Event {
    /// The transaction block that emitted this event. This information is only
    /// available for events from transactions included in a checkpoint.
    ///
    /// For simulated transactions (e.g. dry run), or transactions that have
    /// been just executed but not yet included in a checkpoint this returns
    /// null.
    #[graphql(complexity = "DEFAULT_PAGE_SIZE as usize * (1 + child_complexity)")]
    async fn transaction_block(&self, ctx: &Context<'_>) -> Result<Option<TransactionBlock>> {
        let Some(checkpointed) = &self.checkpointed_info else {
            return Ok(None);
        };
        let key = DigestKey::new(checkpointed.tx_digest, self.checkpoint_viewed_at);

        TransactionBlock::query(ctx, key).await.extend()
    }

    /// The Move module containing some function that when called by
    /// a programmable transaction block (PTB) emitted this event.
    /// For example, if a PTB invokes A::m1::foo, which internally
    /// calls A::m2::emit_event to emit an event,
    /// the sending module would be A::m1.
    #[graphql(complexity = "child_complexity")]
    async fn sending_module(&self, ctx: &Context<'_>) -> Result<Option<MoveModule>> {
        MoveModule::query(
            ctx,
            self.native.package_id.into(),
            self.native.module.as_str(),
            self.checkpoint_viewed_at,
        )
        .await
        .extend()
    }

    /// Address of the sender of the event
    #[graphql(complexity = "child_complexity")]
    async fn sender(&self) -> Result<Option<Address>> {
        if self.native.sender == NativeAddress::ZERO {
            return Ok(None);
        }

        Ok(Some(Address {
            address: self.native.sender.into(),
            checkpoint_viewed_at: self.checkpoint_viewed_at,
        }))
    }

    /// UTC timestamp in milliseconds since epoch (1/1/1970)
    #[graphql(complexity = 0)]
    async fn timestamp(&self) -> Result<Option<DateTime>, Error> {
        if let Some(checkpointed) = &self.checkpointed_info {
            Ok(Some(DateTime::from_ms(checkpointed.timestamp_ms)?))
        } else {
            Ok(None)
        }
    }

    #[graphql(flatten)]
    async fn move_value(&self) -> Result<MoveValue> {
        Ok(MoveValue::new(
            self.native.type_.clone().into(),
            Base64::from(self.native.contents.clone()),
        ))
    }
}

impl Event {
    /// Query the database for a `page` of events. The Page uses the
    /// transaction, event, and checkpoint sequence numbers as the cursor to
    /// determine the correct page of results. The query can optionally be
    /// further `filter`-ed by the `EventFilter`.
    ///
    /// The `checkpoint_viewed_at` parameter represents the checkpoint sequence
    /// number at which this page was queried. Each entity returned in the
    /// connection inherits this checkpoint, so that when viewing that
    /// entity's state, it's as if it's being viewed at this checkpoint.
    ///
    /// The cursors in `page` might also include checkpoint viewed at fields. If
    /// these are set, they take precedence over the checkpoint that
    /// pagination is being conducted in.
    pub(crate) async fn paginate(
        db: &Db,
        page: Page<Cursor>,
        filter: EventFilter,
        checkpoint_viewed_at: u64,
    ) -> Result<Connection<String, Event>, Error> {
        let cursor_viewed_at = page.validate_cursor_consistency()?;
        let checkpoint_viewed_at = cursor_viewed_at.unwrap_or(checkpoint_viewed_at);

        use checkpoints::dsl;
        // Exclusive upperbound, we cannot return newer data than `checkpoint_viewed_at`
        let tx_hi: i64 = db
            .execute(move |conn| {
                conn.first(move || {
                    dsl::checkpoints
                        .select(dsl::network_total_transactions)
                        .filter(dsl::sequence_number.eq(checkpoint_viewed_at as i64))
                })
            })
            .await?;

        // For the only-`transactionDigest` case, we support fallback.
        if let Some(tx_digest) = filter.only_transaction_digest() {
            return Self::paginate_by_tx_digest_with_fallback(
                db,
                page,
                tx_digest,
                checkpoint_viewed_at,
                tx_hi,
            )
            .await;
        }

        // Construct tx and ev sequence number query with table-relevant filters, if
        // they exist. The resulting query will look something like `SELECT
        // tx_sequence_number, event_sequence_number FROM lookup_table WHERE
        // ...`. If no filter is provided we don't need to use any lookup tables
        // and can just query `events` table, as can be seen in the code below.
        let query_constraint = match (filter.sender, &filter.emitting_module, &filter.event_type) {
            (None, None, None) => None,
            (Some(sender), None, None) => Some(select_sender(sender)),
            (sender, None, Some(event_type)) => Some(select_event_type(event_type, sender)),
            (sender, Some(module), None) => Some(select_emit_module(module, sender)),
            (_, Some(_), Some(_)) => {
                return Err(Error::Client(
                    "Filtering by both emitting module and event type is not supported".to_string(),
                ));
            }
        };

        let (prev, next, results) = db
            .execute(move |conn| {
                let (prev, next, mut events): (bool, bool, Vec<StoredEvent>) =
                    if let Some(filter_query) =  query_constraint {
                        let query = add_bounds(filter_query, &filter.transaction_digest, &page, tx_hi);

                        let (prev, next, results) =
                            page.paginate_raw_query::<EvLookup>(conn, checkpoint_viewed_at, query)?;

                        let ev_lookups = results
                            .into_iter()
                            .map(|x| (x.tx, x.ev))
                            .collect::<Vec<(i64, i64)>>();

                        if ev_lookups.is_empty() {
                            return Ok::<_, diesel::result::Error>((prev, next, vec![]));
                        }

                        // Unlike a multi-get on a single column which can be serviced by a query `IN
                        // (...)`, because events have a composite primary key, the query planner tends
                        // to perform a sequential scan when given a list of tuples to lookup. A query
                        // using `UNION ALL` allows us to leverage the index on the composite key.
                        let events = conn.results(move || {
                            // Diesel's DSL does not current support chained `UNION ALL`, so we have to turn
                            // to `RawQuery` here.
                            let query_string = ev_lookups.iter()
                                .map(|&(tx, ev)| {
                                    format!("SELECT * FROM events WHERE tx_sequence_number = {tx} AND event_sequence_number = {ev}")
                                })
                                .collect::<Vec<String>>()
                                .join(" UNION ALL ");

                            query!(query_string).into_boxed()
                        })?;
                        (prev, next, events)
                    } else {
                        // No filter is provided so we add bounds to the basic `SELECT * FROM
                        // events` query and call it a day.
                        let query = add_bounds(query!("SELECT * FROM events"), &filter.transaction_digest, &page, tx_hi);
                        let (prev, next, events_iter) = page.paginate_raw_query::<StoredEvent>(conn, checkpoint_viewed_at, query)?;
                        let events = events_iter.collect::<Vec<StoredEvent>>();
                        (prev, next, events)
                    };

                // UNION ALL does not guarantee order, so we need to sort the results. Whether
                // `first` or `last, the result set is always sorted in ascending order.
                events.sort_by(|a, b| {
                        a.tx_sequence_number.cmp(&b.tx_sequence_number)
                            .then_with(|| a.event_sequence_number.cmp(&b.event_sequence_number))
                });


                Ok::<_, diesel::result::Error>((prev, next, events))
            })
            .await?;

        let mut conn = Connection::new(prev, next);

        // The "checkpoint viewed at" sets a consistent upper bound for the nested
        // queries.
        for stored in results {
            let cursor = stored.cursor(checkpoint_viewed_at).encode_cursor();
            conn.edges.push(Edge::new(
                cursor,
                Event::try_from_stored_event(stored, checkpoint_viewed_at)?,
            ));
        }

        Ok(conn)
    }

    /// Paginates events of a single transaction with fallback support.
    ///
    /// `tx_hi` is the exclusive upperbound on transaction sequence numbers
    async fn paginate_by_tx_digest_with_fallback(
        db: &Db,
        page: Page<Cursor>,
        tx_digest: Digest,
        checkpoint_viewed_at: u64,
        tx_hi: i64,
    ) -> Result<Connection<String, Event>, Error> {
        // Fetch the page plus the rows at the cursors, to later compute
        // `has_next`/`has_prev` via `paginate_results`.
        let min_tx_ev_seq = page
            .after()
            .map_or(Bound::Unbounded, |c| Bound::Included((c.tx, c.e)));
        // Events of transactions at or above `tx_hi` are not visible at
        // `checkpoint_viewed_at`.
        let max_tx_ev_seq = match page.before() {
            Some(c) if c.tx < tx_hi as u64 => Bound::Included((c.tx, c.e)),
            _ => Bound::Excluded((tx_hi as u64, 0)),
        };
        let mut results = db
            .inner
            .query_stored_events_by_tx_digest_with_fallback(
                tx_digest.into(),
                (min_tx_ev_seq, max_tx_ev_seq),
                page.limit() + 2,
                !page.is_from_front(),
            )
            .await
            .map_err(Error::from)?;
        if !page.is_from_front() {
            results.reverse();
        }

        let (prev, next, results) = page.paginate_results(
            results.first().map(|f| f.cursor(checkpoint_viewed_at)),
            results.last().map(|l| l.cursor(checkpoint_viewed_at)),
            results,
        );

        let mut conn = Connection::new(prev, next);
        for stored in results {
            let cursor = stored.cursor(checkpoint_viewed_at).encode_cursor();
            conn.edges.push(Edge::new(
                cursor,
                Event::try_from_stored_event(stored, checkpoint_viewed_at)?,
            ));
        }
        Ok(conn)
    }

    pub(crate) fn try_from_stored_transaction(
        stored_tx: &StoredTransaction,
        idx: usize,
        checkpoint_viewed_at: u64,
    ) -> Result<Self, Error> {
        let Some(serialized_event) = &stored_tx.get_event_at_idx(idx) else {
            return Err(Error::Internal(format!(
                "Could not find event with event_sequence_number {} at transaction {}",
                idx, stored_tx.tx_sequence_number
            )));
        };

        let native_event: NativeEvent = bcs::from_bytes(serialized_event).map_err(|_| {
            Error::Internal(format!(
                "Failed to deserialize event with {} at transaction {}",
                idx, stored_tx.tx_sequence_number
            ))
        })?;

        let tx_digest = Digest::try_from(stored_tx.transaction_digest.as_slice())
            .map_err(|e| Error::Internal(format!("Bad transaction digest on transaction: {e}")))?;
        let checkpointed = CheckpointedEventInfo {
            tx_digest,
            timestamp_ms: stored_tx.timestamp_ms,
        };
        Ok(Self {
            checkpointed_info: Some(checkpointed),
            native: native_event,
            checkpoint_viewed_at,
        })
    }

    pub(crate) fn try_from_stored_event(
        stored: StoredEvent,
        checkpoint_viewed_at: u64,
    ) -> Result<Self, Error> {
        let Some(Some(sender_bytes)) = stored.senders.first() else {
            return Err(Error::Internal("No senders found for event".to_string()));
        };
        let checkpointed = CheckpointedEventInfo::try_from(&stored)?;
        let sender =
            NativeAddress::from_bytes(sender_bytes).map_err(|e| Error::Internal(e.to_string()))?;
        let package_id =
            ObjectId::from_bytes(&stored.package).map_err(|e| Error::Internal(e.to_string()))?;
        let type_ = parse_iota_struct_tag(&stored.event_type)
            .map_err(|e| Error::Internal(e.to_string()))?;
        let module = Identifier::new(&stored.module).map_err(|e| Error::Internal(e.to_string()))?;
        let contents = stored.bcs.clone();
        Ok(Event {
            checkpointed_info: Some(checkpointed),
            native: NativeEvent {
                sender,
                package_id,
                module,
                type_,
                contents,
            },
            checkpoint_viewed_at,
        })
    }

    pub(crate) fn try_from_optimistic_transaction(
        optimistic_tx: &OptimisticTransaction,
        idx: usize,
        checkpoint_viewed_at: u64,
    ) -> Result<Self, Error> {
        let Some(serialized_event) = &optimistic_tx.get_event_at_idx(idx) else {
            return Err(Error::Internal(format!(
                "Could not find event with event_sequence_number {idx} at optimistic transaction {}",
                optimistic_tx.optimistic_sequence_number
            )));
        };

        let native_event: NativeEvent = bcs::from_bytes(serialized_event).map_err(|_| {
            Error::Internal(format!(
                "Failed to deserialize event with {idx} at optimistic transaction {}",
                optimistic_tx.optimistic_sequence_number
            ))
        })?;

        Ok(Self {
            checkpointed_info: None,
            native: native_event,
            checkpoint_viewed_at,
        })
    }
}
