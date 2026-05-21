// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use async_graphql::*;
use diesel::{ExpressionMethods, QueryDsl, QueryResult};
use iota_indexer::schema::{checkpoints, watermarks};

use crate::{
    data::{Conn, Db, DbConnection, QueryExecutor},
    error::Error,
    types::checkpoint::{Checkpoint, CheckpointId},
};
#[derive(Clone, Debug, PartialEq, Eq, Copy)]
pub(crate) struct AvailableRange {
    pub first: u64,
    pub last: u64,
}

/// Range of checkpoints that the RPC is guaranteed to produce a consistent
/// response for.
#[Object]
impl AvailableRange {
    async fn first(&self, ctx: &Context<'_>) -> Result<Option<Checkpoint>> {
        Checkpoint::query(ctx, CheckpointId::by_seq_num(self.first), self.last)
            .await
            .extend()
    }

    async fn last(&self, ctx: &Context<'_>) -> Result<Option<Checkpoint>> {
        Checkpoint::query(ctx, CheckpointId::by_seq_num(self.last), self.last)
            .await
            .extend()
    }
}

impl AvailableRange {
    /// Look up the available range when viewing the data consistently at
    /// `checkpoint_viewed_at`. Uses the executor's configured
    /// `max_available_range`.
    pub(crate) async fn query(db: &Db, checkpoint_viewed_at: u64) -> Result<Self, Error> {
        let max_available_range = db.max_available_range;
        let Some(range): Option<Self> = db
            .execute(move |conn| Self::result(conn, checkpoint_viewed_at, max_available_range))
            .await
            .map_err(|e| Error::Internal(format!("Failed to fetch available range: {e}")))?
        else {
            return Err(Error::Client(format!(
                "Requesting data at checkpoint {checkpoint_viewed_at}, outside the available \
                 range.",
            )));
        };

        Ok(range)
    }

    /// Computes the backward-diff retention window. Returns `Some(range)`
    /// when `checkpoint_viewed_at` falls inside it, `None` otherwise. The
    /// lower bound is the greater of the backward history watermark
    /// (`min_available_cp`) and `latest_checkpoint - max_available_range`; the
    /// upper bound is `checkpoint_viewed_at` itself, so the returned range
    /// never extends past the request's captured watermark.
    ///
    /// `max_available_range` caps how far back a consistent view can be
    /// requested, for performance reasons (the further back, the more
    /// backward history entries to traverse).
    pub(crate) fn result(
        conn: &mut Conn,
        checkpoint_viewed_at: u64,
        max_available_range: u64,
    ) -> QueryResult<Option<Self>> {
        use checkpoints::dsl as cp;
        use watermarks::dsl as wm;

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

        let watermark_first: Option<i64> = conn
            .results(|| {
                wm::watermarks
                    .filter(wm::entity.eq(crate::backward_view::BACKWARD_HISTORY_WATERMARK_ENTITY))
                    .select(wm::min_available_cp)
                    .limit(1)
                    .into_boxed()
            })?
            .into_iter()
            .next();

        let last = last.unwrap_or(0) as u64;
        let watermark_first = watermark_first.unwrap_or(0) as u64;
        let lag_first = last.saturating_sub(max_available_range);
        let first = watermark_first.max(lag_first);

        if checkpoint_viewed_at < first || checkpoint_viewed_at > last {
            return Ok(None);
        }
        // Cap `last` to `checkpoint_viewed_at` so a single request never
        // reports a range extending past its captured watermark.
        Ok(Some(Self {
            first,
            last: checkpoint_viewed_at,
        }))
    }

    pub(crate) fn is_checkpoint_in_backward_history_range(
        conn: &mut Conn,
        checkpoint_viewed_at: u64,
        max_available_range: u64,
    ) -> QueryResult<bool> {
        Ok(Self::result(conn, checkpoint_viewed_at, max_available_range)?.is_some())
    }
}
