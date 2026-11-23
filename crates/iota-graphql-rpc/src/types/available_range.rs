// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use async_graphql::*;
use diesel::{ExpressionMethods, QueryDsl, QueryResult};
use iota_indexer::{
    ingestion::common::CommitterTables,
    schema::{checkpoints, watermarks},
};

use crate::{
    backward_view::BACKWARD_HISTORY_WATERMARK_ENTITY,
    data::{Conn, Db, DbConnection, QueryExecutor},
    error::Error,
    types::checkpoint::{Checkpoint, CheckpointId},
};

/// Represents the view on the ledger while querying for objects.
#[derive(Clone, Copy, Debug)]
pub(crate) struct LedgerView {
    /// Whether the view is live or consistent.
    pub kind: LedgerViewKind,
    /// The lowest checkpoint of a consistent view, or `None` for a live view.
    pub min: Option<u64>,
    /// The latest checkpoint of a live view, or the highest checkpoint of a
    /// consistent view.
    pub max: u64,
}

impl LedgerView {
    /// Resolves the view on the ledger from the database.
    ///
    /// The criterion is the state of the history table (`objects_backward_history`).
    /// If it is in sync, the view is consistent. Otherwise, the view falls back to the
    /// live rolling view on the ledger.
    ///
    /// Synchronization is determined by comparing the watermark of the history table
    /// with the watermark of the table storing checkpoints.
    ///
    /// In a consistent view, the lower limit of the range is the greater of the
    /// backward history watermark (`min_available_cp`) and
    /// `latest_checkpoint - max_available_range`. The upper limit is the latest
    /// checkpoint. `max_available_range` limits how far back a consistent view
    /// can go, for performance reasons: a view further back must read more
    /// backward history entries.
    ///
    /// # Errors
    ///
    /// Fails if a query on the database fails.
    pub(crate) fn resolve(conn: &mut Conn, max_available_range: u64) -> QueryResult<Self> {
        use watermarks::dsl as wm;

        let checkpoints_entity = CommitterTables::Checkpoints.as_ref();
        let watermarks: Vec<(i64, i64)> = conn.results(|| {
            wm::watermarks
                .filter(wm::entity.eq_any([BACKWARD_HISTORY_WATERMARK_ENTITY, checkpoints_entity]))
                // The watermark of the history table comes first.
                .order(wm::entity.eq(BACKWARD_HISTORY_WATERMARK_ENTITY).desc())
                .select((wm::min_available_cp, wm::max_committed_cp))
                .into_boxed()
        })?;
        let latest_checkpoint = Self::latest_checkpoint(conn)?;

        match watermarks.as_slice() {
            [(min_available_cp, max_history_cp), (_, max_checkpoints_cp)]
                if max_history_cp == max_checkpoints_cp =>
            {
                let retention_window_start_cp =
                    latest_checkpoint.saturating_sub(max_available_range);
                Ok(Self {
                    kind: LedgerViewKind::Consistent,
                    min: Some((*min_available_cp as u64).max(retention_window_start_cp)),
                    max: latest_checkpoint,
                })
            }
            _ => Ok(Self {
                kind: LedgerViewKind::Live,
                min: None,
                max: latest_checkpoint,
            }),
        }
    }

    /// Returns the checkpoint at which a query reads objects, or `None` if
    /// `checkpoint_viewed_at` is outside the available range.
    ///
    /// In a consistent view, the result is `checkpoint_viewed_at`.
    ///
    /// In a live view, which can occur when the indexer operates on the
    /// `Basic` operational level, the result is the latest checkpoint.
    pub(crate) fn resolve_checkpoint(self, checkpoint_viewed_at: u64) -> Option<u64> {
        match self.kind {
            LedgerViewKind::Live => Some(self.max),
            LedgerViewKind::Consistent => AvailableRange::from(self)
                .contains(checkpoint_viewed_at)
                .then_some(checkpoint_viewed_at),
        }
    }

    /// Looks up the view on the ledger in the database.
    ///
    /// In a consistent view, the upper limit of the range becomes
    /// `checkpoint_viewed_at`. A live view does not change.
    ///
    /// # Errors
    ///
    /// Fails if a query on the database fails, or if, in a consistent view,
    /// `checkpoint_viewed_at` is outside the available range.
    pub(crate) async fn query(db: &Db, checkpoint_viewed_at: u64) -> Result<Self, Error> {
        let max_available_range = db.max_available_range;
        let mut ledger_view = db
            .execute(move |conn| Self::resolve(conn, max_available_range))
            .await
            .map_err(|e| Error::Internal(format!("Failed to fetch the ledger view: {e}")))?;

        match ledger_view.kind {
            LedgerViewKind::Live => Ok(ledger_view),
            LedgerViewKind::Consistent => {
                if !AvailableRange::from(ledger_view).contains(checkpoint_viewed_at) {
                    return Err(Error::Client(format!(
                        "Requesting data at checkpoint {checkpoint_viewed_at}, outside the \
                         available range.",
                    )));
                }

                // A single request must not report a range past its captured watermark.
                ledger_view.max = checkpoint_viewed_at;
                Ok(ledger_view)
            }
        }
    }

    /// Returns the sequence number of the latest fully indexed checkpoint in the database,
    /// or 0 if the database has no checkpoints.
    ///
    /// # Errors
    ///
    /// Fails if the query on the database fails.
    fn latest_checkpoint(conn: &mut Conn) -> QueryResult<u64> {
        use checkpoints::dsl as cp;

        let last: Option<i64> = conn
            .results(|| {
                cp::checkpoints
                    .select(cp::sequence_number)
                    .order(cp::sequence_number.desc())
                    .limit(1)
                    .into_boxed()
            })?
            .into_iter()
            .next();

        Ok(last.unwrap_or(0) as u64)
    }
}

/// View on the ledger that object queries read.
///
/// This applies to objects, coins, balances, dynamic fields and name registrations.
///
/// A `LIVE` view always reads the latest state of objects at `max`. This implies that
/// two consecutive pages of results might not reflect the same state of the ledger.
///
/// A `CONSISTENT` view reads a pinned snapshot of objects at any checkpoint
/// from `min` to `max`.
#[Object]
impl LedgerView {
    /// Whether the view is live or consistent.
    async fn kind(&self) -> LedgerViewKind {
        self.kind
    }

    /// The lowest checkpoint of a consistent view, or `null` for a live view.
    async fn min(&self, ctx: &Context<'_>) -> Result<Option<Checkpoint>> {
        let Some(min) = self.min else {
            return Ok(None);
        };
        let id = CheckpointId::by_seq_num(min).extend()?;
        Checkpoint::query(ctx, id, self.max).await.extend()
    }

    /// The latest checkpoint of a live view, or the highest checkpoint of a
    /// consistent view.
    async fn max(&self, ctx: &Context<'_>) -> Result<Option<Checkpoint>> {
        let id = CheckpointId::by_seq_num(self.max).extend()?;
        Checkpoint::query(ctx, id, self.max).await.extend()
    }
}

/// The kind of a view on the ledger.
#[derive(Enum, Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum LedgerViewKind {
    /// Rolling view over the latest state of objects.
    Live,
    /// Consistent view at any checkpoint in a range.
    Consistent,
}

#[derive(Clone, Debug, PartialEq, Eq, Copy)]
pub(crate) struct AvailableRange {
    pub first: u64,
    pub last: u64,
}

/// Range of checkpoints that object queries can be viewed at.
///
/// For consistent view, a query paginated with a cursor is viewed at
/// the checkpoint the cursor was issued at, so the cursor expires once that
/// checkpoint leaves the range.
///
/// For live views, the ranges resolves into the latest checkpoint with available
/// data on the db.
#[Object]
impl AvailableRange {
    async fn first(&self, ctx: &Context<'_>) -> Result<Option<Checkpoint>> {
        let id = CheckpointId::by_seq_num(self.first).extend()?;
        Checkpoint::query(ctx, id, self.last).await.extend()
    }

    async fn last(&self, ctx: &Context<'_>) -> Result<Option<Checkpoint>> {
        let id = CheckpointId::by_seq_num(self.last).extend()?;
        Checkpoint::query(ctx, id, self.last).await.extend()
    }
}

impl AvailableRange {
    /// Looks up the available range in the database.
    ///
    /// In a consistent view the method aligns it with the view at `checkpoint_viewed_at`.
    ///
    /// In a live view, the range contains only the latest checkpoint.
    ///
    /// # Errors
    ///
    /// Fails if a query on the database fails, or if, in a consistent view,
    /// `checkpoint_viewed_at` is outside the available range.
    pub(crate) async fn query(db: &Db, checkpoint_viewed_at: u64) -> Result<Self, Error> {
        LedgerView::query(db, checkpoint_viewed_at)
            .await
            .map(Self::from)
    }

    /// Checks whether the given `checkpoint` is within the range.
    pub(crate) fn contains(&self, checkpoint: u64) -> bool {
        checkpoint >= self.first && checkpoint <= self.last
    }
}

impl From<LedgerView> for AvailableRange {
    /// Converts a view on the ledger into a range of checkpoints.
    ///
    /// A live view becomes a range that contains only its latest checkpoint.
    fn from(ledger_view: LedgerView) -> Self {
        Self {
            first: ledger_view.min.unwrap_or(ledger_view.max),
            last: ledger_view.max,
        }
    }
}
