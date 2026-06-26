// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::{BTreeMap, BTreeSet, HashMap};

use async_graphql::{
    connection::{Connection, CursorType, Edge},
    dataloader::Loader,
    *,
};
use diesel::{ExpressionMethods, OptionalExtension, QueryDsl};
use fastcrypto::encoding::{Base58, Encoding};
use iota_indexer::{
    models::checkpoints::StoredCheckpoint, pruning::CommitterTables, schema::checkpoints,
};
use iota_types::messages_checkpoint::CheckpointDigest;
use serde::{Deserialize, Serialize};

use crate::{
    config::DEFAULT_PAGE_SIZE,
    connection::ScanConnection,
    consistency::Checkpointed,
    data::{Conn, DataLoader, Db, DbConnection, QueryExecutor},
    error::Error,
    types::{
        base64::Base64,
        cursor::{self, Page, ScanLimited, Target},
        date_time::DateTime,
        digest::Digest,
        epoch::Epoch,
        gas::GasCostSummary,
        transaction_block::{self, TransactionBlock, TransactionBlockFilter},
        uint53::UInt53,
    },
};

/// Filter either by the digest, or the sequence number, or neither, to get the
/// latest checkpoint.
#[derive(Default, InputObject)]
pub(crate) struct CheckpointId {
    pub digest: Option<Digest>,
    pub sequence_number: Option<UInt53>,
}

/// `DataLoader` key for fetching a `Checkpoint` by its sequence number,
/// constrained by a consistency cursor.
#[derive(Copy, Clone, Hash, Eq, PartialEq, Debug)]
struct SeqNumKey {
    pub sequence_number: u64,
    /// The digest is not used for fetching, but is used as an additional
    /// filter, to correctly implement a request that sets both a sequence
    /// number and a digest.
    pub digest: Option<Digest>,
    pub checkpoint_viewed_at: u64,
}

/// DataLoader key for fetching a `Checkpoint` by its digest, optionally
/// constrained by a consistency cursor.
#[derive(Copy, Clone, Hash, Eq, PartialEq, Debug)]
struct DigestKey {
    pub digest: Digest,
    pub checkpoint_viewed_at: u64,
}

#[derive(Clone)]
pub(crate) struct Checkpoint {
    /// Representation of transaction data in the Indexer's Store. The indexer
    /// stores the transaction data and its effects together, in one table.
    pub stored: StoredCheckpoint,
    /// The checkpoint_sequence_number at which this was viewed at.
    pub checkpoint_viewed_at: u64,
}

pub(crate) type Cursor = cursor::JsonCursor<CheckpointCursor>;

/// The cursor returned for each `Checkpoint` in a connection's page of results.
/// The `checkpoint_viewed_at` will set the consistent upper bound for
/// subsequent queries made on this cursor.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
pub(crate) struct CheckpointCursor {
    /// The checkpoint sequence number this was viewed at.
    #[serde(rename = "c")]
    pub checkpoint_viewed_at: u64,
    #[serde(rename = "s")]
    pub sequence_number: u64,
}

/// Checkpoints contain finalized transactions and are used for node
/// synchronization and global transaction ordering.
#[Object]
impl Checkpoint {
    /// A 32-byte hash that uniquely identifies the checkpoint contents, encoded
    /// in Base58. This hash can be used to verify checkpoint contents by
    /// checking signatures against the committee, Hashing contents to match
    /// digest, and checking that the previous checkpoint digest matches.
    #[graphql(complexity = 0)]
    async fn digest(&self) -> Result<String> {
        Ok(self.digest_impl().extend()?.to_base58())
    }

    /// This checkpoint's position in the total order of finalized checkpoints,
    /// agreed upon by consensus.
    #[graphql(complexity = 0)]
    async fn sequence_number(&self) -> UInt53 {
        self.sequence_number_impl().into()
    }

    /// The timestamp at which the checkpoint is agreed to have happened
    /// according to consensus. Transactions that access time in this
    /// checkpoint will observe this timestamp.
    #[graphql(complexity = 0)]
    async fn timestamp(&self) -> Result<DateTime> {
        DateTime::from_ms(self.stored.timestamp_ms).extend()
    }

    /// This is an aggregation of signatures from a quorum of validators for the
    /// checkpoint proposal.
    #[graphql(complexity = 0)]
    async fn validator_signatures(&self) -> Base64 {
        Base64::from(&self.stored.validator_signature)
    }

    /// The digest of the checkpoint at the previous sequence number.
    #[graphql(complexity = 0)]
    async fn previous_checkpoint_digest(&self) -> Option<String> {
        self.stored
            .previous_checkpoint_digest
            .as_ref()
            .map(Base58::encode)
    }

    /// The total number of transaction blocks in the network by the end of this
    /// checkpoint.
    #[graphql(complexity = 0)]
    async fn network_total_transactions(&self) -> Option<UInt53> {
        Some(self.network_total_transactions_impl().into())
    }

    /// The computation cost, storage cost, storage rebate, and non-refundable
    /// storage fee accumulated during this epoch, up to and including this
    /// checkpoint. These values increase monotonically across checkpoints
    /// in the same epoch, and reset on epoch boundaries.
    #[graphql(complexity = 0)]
    async fn rolling_gas_summary(&self) -> Option<GasCostSummary> {
        Some(GasCostSummary {
            computation_cost: self.stored.computation_cost as u64,
            computation_cost_burned: self.stored.computation_cost_burned(),
            storage_cost: self.stored.storage_cost as u64,
            storage_rebate: self.stored.storage_rebate as u64,
            non_refundable_storage_fee: self.stored.non_refundable_storage_fee as u64,
        })
    }

    /// The epoch this checkpoint is part of.
    async fn epoch(&self, ctx: &Context<'_>) -> Result<Option<Epoch>> {
        Epoch::query(
            ctx,
            Some(self.stored.epoch as u64),
            self.checkpoint_viewed_at,
        )
        .await
        .extend()
    }

    /// Transactions in this checkpoint.
    ///
    /// `scanLimit` restricts the number of candidate transactions scanned when
    /// gathering a page of results. It is required for queries that apply
    /// more than two complex filters (on function, kind, sender, recipient,
    /// input object, changed object, or ids), and can be at most
    /// `serviceConfig.maxScanLimit`.
    ///
    /// When the scan limit is reached the page will be returned even if it has
    /// fewer than `first` results when paginating forward (`last` when
    /// paginating backwards). If there are more transactions to scan,
    /// `pageInfo.hasNextPage` (or `pageInfo.hasPreviousPage`) will be set to
    /// `true`, and `PageInfo.endCursor` (or `PageInfo.startCursor`) will be set
    /// to the last transaction that was scanned as opposed to the last (or
    /// first) transaction in the page.
    ///
    /// Requesting the next (or previous) page after this cursor will resume the
    /// search, scanning the next `scanLimit` many transactions in the
    /// direction of pagination, and so on until all transactions in the
    /// scanning range have been visited.
    ///
    /// By default, the scanning range consists of all transactions in this
    /// checkpoint.
    #[graphql(
        complexity = "first.or(last).unwrap_or(DEFAULT_PAGE_SIZE as u64) as usize * child_complexity"
    )]
    async fn transaction_blocks(
        &self,
        ctx: &Context<'_>,
        first: Option<u64>,
        after: Option<transaction_block::Cursor>,
        last: Option<u64>,
        before: Option<transaction_block::Cursor>,
        filter: Option<TransactionBlockFilter>,
        scan_limit: Option<u64>,
    ) -> Result<ScanConnection<String, TransactionBlock>> {
        let page = Page::from_params(ctx.data_unchecked(), first, after, last, before)?;

        let Some(filter) = filter
            .unwrap_or_default()
            .intersect(TransactionBlockFilter {
                at_checkpoint: Some(UInt53::from(self.stored.sequence_number as u64)),
                ..Default::default()
            })
        else {
            return Ok(ScanConnection::new(false, false));
        };

        TransactionBlock::paginate(ctx, page, filter, self.checkpoint_viewed_at, scan_limit)
            .await
            .extend()
    }
}

impl CheckpointId {
    pub(crate) fn by_seq_num(seq_num: u64) -> Self {
        CheckpointId {
            sequence_number: Some(seq_num.into()),
            digest: None,
        }
    }
}

impl Checkpoint {
    pub(crate) fn sequence_number_impl(&self) -> u64 {
        self.stored.sequence_number as u64
    }

    pub(crate) fn network_total_transactions_impl(&self) -> u64 {
        self.stored.network_total_transactions as u64
    }

    pub(crate) fn digest_impl(&self) -> Result<CheckpointDigest, Error> {
        CheckpointDigest::from_bytes(self.stored.checkpoint_digest.clone())
            .map_err(|e| Error::Internal(format!("Failed to deserialize checkpoint digest: {e}")))
    }

    /// Look up a `Checkpoint` in the database, filtered by either sequence
    /// number or digest. If both filters are supplied they will both be
    /// applied. If none are supplied, the latest checkpoint is fetched.
    pub(crate) async fn query(
        ctx: &Context<'_>,
        filter: CheckpointId,
        checkpoint_viewed_at: u64,
    ) -> Result<Option<Self>, Error> {
        match filter {
            CheckpointId {
                sequence_number: Some(sequence_number),
                digest,
            } => {
                let DataLoader(dl) = ctx.data_unchecked();
                dl.load_one(SeqNumKey {
                    sequence_number: sequence_number.into(),
                    digest,
                    checkpoint_viewed_at,
                })
                .await
            }

            CheckpointId {
                sequence_number: None,
                digest: Some(digest),
            } => {
                let DataLoader(dl) = ctx.data_unchecked();
                dl.load_one(DigestKey {
                    digest,
                    checkpoint_viewed_at,
                })
                .await
            }

            CheckpointId {
                sequence_number: None,
                digest: None,
            } => Checkpoint::query_latest_at(ctx.data_unchecked(), checkpoint_viewed_at).await,
        }
    }

    /// Look up the latest `Checkpoint` from the database, optionally filtered
    /// by a consistency cursor (querying for a consistency cursor in the
    /// past looks for the latest checkpoint as of that cursor).
    async fn query_latest_at(db: &Db, checkpoint_viewed_at: u64) -> Result<Option<Self>, Error> {
        use checkpoints::dsl;

        let stored: Option<StoredCheckpoint> = db
            .execute(move |conn| {
                conn.first(move || {
                    dsl::checkpoints
                        .filter(dsl::sequence_number.le(checkpoint_viewed_at as i64))
                        .order_by(dsl::sequence_number.desc())
                })
                .optional()
            })
            .await
            .map_err(|e| Error::Internal(format!("Failed to fetch checkpoint: {e}")))?;

        Ok(stored.map(|stored| Checkpoint {
            stored,
            checkpoint_viewed_at,
        }))
    }

    /// Look up a `Checkpoint` in the database and retrieve its `timestamp_ms`
    /// field. This method takes a connection, so that it can be used within
    /// a transaction.
    pub(crate) fn query_timestamp(
        conn: &mut Conn<'_>,
        seq_num: u64,
    ) -> Result<u64, diesel::result::Error> {
        use checkpoints::dsl;

        let stored: i64 = conn.first(|| {
            dsl::checkpoints
                .select(dsl::timestamp_ms)
                .filter(dsl::sequence_number.eq(seq_num as i64))
        })?;

        Ok(stored as u64)
    }

    /// Returns the inclusive `[lo, hi]` checkpoint sequence-number range to
    /// paginate over, given the `checkpoint_viewed_at` and optional `epoch`
    /// filter. Returns `None` when `filter` targets an epoch that does not
    /// exist.
    ///
    /// The `epochs` table is never pruned but an in-progress epoch's
    /// `last_checkpoint_id` is NULL - in that case the upper bound is
    /// capped at `checkpoint_viewed_at`.
    async fn pagination_range(
        db: &Db,
        filter: Option<u64>,
        checkpoint_viewed_at: u64,
    ) -> Result<Option<(u64, u64)>, Error> {
        let Some(epoch) = filter else {
            return Ok(Some((0, checkpoint_viewed_at)));
        };

        let row: Option<(i64, Option<i64>)> = db
            .execute(move |conn| {
                use iota_indexer::schema::epochs::dsl as e;
                conn.first(move || {
                    e::epochs
                        .select((e::first_checkpoint_id, e::last_checkpoint_id))
                        .filter(e::epoch.eq(epoch as i64))
                })
                .optional()
            })
            .await
            .map_err(|err| Error::Internal(format!("Failed to fetch epoch range: {err}")))?;

        Ok(row.map(|(first, last)| {
            let hi = last
                .map(|l| std::cmp::min(l as u64, checkpoint_viewed_at))
                .unwrap_or(checkpoint_viewed_at);
            (first as u64, hi)
        }))
    }

    /// Query the database for a `page` of checkpoints. The Page uses the
    /// checkpoint sequence number of the stored checkpoint and the
    /// checkpoint at which this was viewed at as the cursor, and
    /// can optionally be further `filter`-ed by an epoch number (to only return
    /// checkpoints within that epoch).
    ///
    /// The `checkpoint_viewed_at` parameter represents the checkpoint sequence
    /// number at which this page was queried for. Each entity returned in
    /// the connection will inherit this checkpoint, so that when viewing
    /// that entity's state, it will be from the reference of this
    /// checkpoint_viewed_at parameter.
    ///
    /// If the `Page<Cursor>` is set, then this function will defer to the
    /// `checkpoint_viewed_at` in the cursor if they are consistent.
    ///
    /// Specifying cursor or requesting epoch from the pruned range will result
    /// in an error.
    pub(crate) async fn paginate(
        db: &Db,
        page: Page<Cursor>,
        filter: Option<u64>,
        checkpoint_viewed_at: u64,
    ) -> Result<Connection<String, Checkpoint>, Error> {
        let cursor_viewed_at = page.validate_cursor_consistency()?;
        let checkpoint_viewed_at = cursor_viewed_at.unwrap_or(checkpoint_viewed_at);

        if page.limit() == 0 {
            return Ok(Connection::new(false, false));
        }

        let Some((mut absolute_lo_incl, absolute_hi_incl)) =
            Self::pagination_range(db, filter, checkpoint_viewed_at).await?
        else {
            return Ok(Connection::new(false, false));
        };

        // Without a fallback, anything below the pruning watermark is unreachable
        if !db.inner.is_fallback_enabled() {
            if let Some(lowest_unpruned_cp) = db
                .inner
                .watermark_cache()
                .get_lowest_available_cp_for_tables(&[CommitterTables::Checkpoints])
                .map(|w| w as u64)
            {
                absolute_lo_incl = absolute_lo_incl.max(lowest_unpruned_cp);
            }
        }

        let available_range = absolute_lo_incl..=absolute_hi_incl;

        if available_range.is_empty() {
            return Err(Error::DataPruned(
                "all checkpoints in the requested range have been pruned".into(),
            ));
        }

        // Reject cursors outside the available range. Below the lower bound means data
        // was pruned in the middle of pagination; above the upper bound means the
        // cursor is malformed.
        let validate_cursor = |name: &str, cursor: Option<&Cursor>| -> Result<Option<u64>, Error> {
            let Some(cursor) = cursor else {
                return Ok(None);
            };
            let seq = cursor.sequence_number;
            if seq < absolute_lo_incl {
                return Err(Error::DataPruned(format!(
                    "`{name}` cursor (seq {seq}) is below the available range {available_range:?}"
                )));
            }
            if seq > absolute_hi_incl {
                return Err(Error::Client(format!(
                    "`{name}` cursor (seq {seq}) is above the available range {available_range:?}"
                )));
            }
            Ok(Some(seq))
        };
        // Narrow the range using cursors, keeping in mind cursors are exclusive.
        let page_lo_incl = validate_cursor("after", page.after())?
            .map_or(absolute_lo_incl, |s| s.saturating_add(1));
        let page_hi_incl = validate_cursor("before", page.before())?
            .map_or(absolute_hi_incl, |s| s.saturating_sub(1));

        let page_range = page_lo_incl..=page_hi_incl;
        if page_range.is_empty() {
            return Ok(Connection::new(false, false));
        }

        // Take `limit` sequence numbers from the appropriate end of the page range.
        let limit = page.limit();
        let picked_seqs: Vec<u64> = if page.is_from_front() {
            page_range.take(limit).collect()
        } else {
            page_range.rev().take(limit).collect()
        };
        let mut all_rows: Vec<StoredCheckpoint> = db
            .inner
            .get_stored_checkpoints_by_seqs_with_fallback(picked_seqs.clone())
            .await
            .map_err(|err| Error::Internal(format!("Failed to fetch checkpoints: {err}")))?
            .into_iter()
            .flatten()
            .collect();
        all_rows.sort_by_key(|s| s.sequence_number);

        // We validated the available range earlier, unpruned range should be present in
        // the DB, rest should be present in fallback KV if configured. In such case we
        // expect all checkpoints to be returned.
        if all_rows.len() < picked_seqs.len() {
            let picked: BTreeSet<u64> = picked_seqs.iter().copied().collect();
            let returned: BTreeSet<u64> =
                all_rows.iter().map(|r| r.sequence_number as u64).collect();
            let misses: Vec<u64> = picked.difference(&returned).copied().collect();
            return Err(Error::Internal(format!(
                "checkpoints {misses:?} expected to be available but not found"
            )));
        }

        let fetched_lo = all_rows.first().expect("checked non-empty").sequence_number as u64;
        let fetched_hi = all_rows.last().expect("checked non-empty").sequence_number as u64;
        let has_prev = fetched_lo > absolute_lo_incl;
        let has_next = fetched_hi < absolute_hi_incl;

        let mut conn = Connection::new(has_prev, has_next);
        for stored in all_rows {
            let cursor = stored.cursor(checkpoint_viewed_at).encode_cursor();
            conn.edges.push(Edge::new(
                cursor,
                Checkpoint {
                    stored,
                    checkpoint_viewed_at,
                },
            ));
        }

        Ok(conn)
    }
}

impl Target<Cursor> for StoredCheckpoint {
    fn cursor(&self, checkpoint_viewed_at: u64) -> Cursor {
        Cursor::new(CheckpointCursor {
            checkpoint_viewed_at,
            sequence_number: self.sequence_number as u64,
        })
    }
}

impl Checkpointed for Cursor {
    fn checkpoint_viewed_at(&self) -> u64 {
        self.checkpoint_viewed_at
    }
}

impl ScanLimited for Cursor {}

impl Loader<SeqNumKey> for Db {
    type Value = Checkpoint;
    type Error = Error;

    async fn load(&self, keys: &[SeqNumKey]) -> Result<HashMap<SeqNumKey, Checkpoint>, Error> {
        // Drop keys querying for a checkpoint after their own consistency cursor.
        let seqs: Vec<u64> = keys
            .iter()
            .filter(|key| key.checkpoint_viewed_at >= key.sequence_number)
            .map(|key| key.sequence_number)
            .collect();

        let rows = self
            .inner
            .get_stored_checkpoints_by_seqs_with_fallback(seqs.clone())
            .await
            .map_err(|e| Error::Internal(format!("Failed to fetch checkpoints: {e}")))?;

        let checkpoint_id_to_stored: BTreeMap<u64, StoredCheckpoint> = seqs
            .into_iter()
            .zip(rows)
            .filter_map(|(seq, row)| row.map(|stored| (seq, stored)))
            .collect();

        Ok(keys
            .iter()
            .filter_map(|key| {
                let stored = checkpoint_id_to_stored.get(&key.sequence_number).cloned()?;
                let checkpoint = Checkpoint {
                    stored,
                    checkpoint_viewed_at: key.checkpoint_viewed_at,
                };

                let digest = &checkpoint.stored.checkpoint_digest;
                if matches!(key.digest, Some(d) if d.as_slice() != digest) {
                    None
                } else {
                    Some((*key, checkpoint))
                }
            })
            .collect())
    }
}

impl Loader<DigestKey> for Db {
    type Value = Checkpoint;
    type Error = Error;

    async fn load(&self, keys: &[DigestKey]) -> Result<HashMap<DigestKey, Checkpoint>, Error> {
        let digests: Vec<CheckpointDigest> = keys.iter().map(|key| key.digest.into()).collect();

        let rows = self
            .inner
            .get_stored_checkpoints_by_digests_with_fallback(digests.clone())
            .await
            .map_err(|e| Error::Internal(format!("Failed to fetch checkpoints: {e}")))?;

        let checkpoint_id_to_stored: BTreeMap<Vec<u8>, StoredCheckpoint> = digests
            .into_iter()
            .zip(rows)
            .filter_map(|(digest, row)| row.map(|stored| (digest.inner().to_vec(), stored)))
            .collect();

        Ok(keys
            .iter()
            .filter_map(|key| {
                let DigestKey {
                    digest,
                    checkpoint_viewed_at,
                } = *key;

                let stored = checkpoint_id_to_stored.get(digest.as_slice()).cloned()?;

                let checkpoint = Checkpoint {
                    stored,
                    checkpoint_viewed_at,
                };

                // Filter by key's checkpoint viewed at here. Doing this in memory because it
                // should be quite rare that this query actually filters
                // something, but encoding it in SQL is complicated.
                let seq_num = checkpoint.stored.sequence_number as u64;
                (checkpoint_viewed_at >= seq_num).then_some((*key, checkpoint))
            })
            .collect())
    }
}
