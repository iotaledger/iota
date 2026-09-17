// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
use std::collections::BTreeMap;

use futures::{StreamExt, stream::ReadyChunks};
use iota_metrics::metered_channel::ReceiverStream;
use tap::tap::TapFallible;
use tracing::{error, info, instrument};

use crate::{
    account_key_events::AccountKeyLinkOp,
    ingestion::common::{
        persist::{CHECKPOINT_COMMIT_BATCH_SIZE, CommitterTables, CommitterWatermark},
        prepare::CheckpointObjectChanges,
    },
    metrics::IndexerMetrics,
    models::{
        account_key_links::StoredAccountKeyLink,
        claimed_accounts::StoredClaimedAccount,
        display::StoredDisplay,
        epoch::{EndOfEpochUpdate, StartOfEpochUpdate},
        obj_indices::StoredObjectVersion,
        objects::StoredBackwardHistoryObject,
    },
    store::{IndexerStore, PgIndexerStore},
    types::{
        EventIndex, IndexedCheckpoint, IndexedDeletedObject, IndexedEvent, IndexedObject,
        IndexedPackage, IndexedTransaction, IndexerResult, TxIndex,
    },
};
#[derive(Debug)]
pub(crate) struct CheckpointDataToCommit {
    pub(crate) checkpoint: IndexedCheckpoint,
    pub(crate) transactions: Vec<IndexedTransaction>,
    pub(crate) events: Vec<IndexedEvent>,
    pub(crate) event_indices: Vec<EventIndex>,
    pub(crate) tx_indices: Vec<TxIndex>,
    pub(crate) displays: BTreeMap<String, StoredDisplay>,
    pub(crate) object_changes: CheckpointObjectChanges,
    pub(crate) backward_history_changes: Vec<StoredBackwardHistoryObject>,
    pub(crate) object_versions: Vec<StoredObjectVersion>,
    pub(crate) packages: Vec<IndexedPackage>,
    pub(crate) epoch: Option<EpochToCommit>,
    pub(crate) account_key_link_ops: Vec<AccountKeyLinkOp>,
    pub(crate) claimed_accounts: Vec<StoredClaimedAccount>,
}

/// Collapses `rows` to the last one seen per key.
///
/// The output is ordered by key so that an ingestion run produces the same
/// write set regardless of how the batch was chunked.
fn collapse_last_write_wins<T, K: Ord>(
    rows: impl Iterator<Item = T>,
    key: impl Fn(&T) -> K,
) -> Vec<T> {
    let mut latest = std::collections::BTreeMap::new();
    for row in rows {
        latest.insert(key(&row), row);
    }
    latest.into_values().collect()
}

#[derive(Clone, Debug, Default)]
pub struct TransactionObjectChangesToCommit {
    pub changed_objects: Vec<IndexedObject>,
    pub deleted_objects: Vec<IndexedDeletedObject>,
}

#[derive(Clone, Debug)]
pub struct EpochToCommit {
    pub(crate) last_epoch: Option<EndOfEpochUpdate>,
    pub(crate) new_epoch: StartOfEpochUpdate,
}

pub(crate) struct PrimaryWriter {
    state: PgIndexerStore,
    metrics: IndexerMetrics,
    pub stream: ReadyChunks<ReceiverStream<CheckpointDataToCommit>>,
    pub checkpoint_commit_batch_size: usize,
}

impl PrimaryWriter {
    pub fn new(
        state: PgIndexerStore,
        metrics: IndexerMetrics,
        tx_indexing_receiver: iota_metrics::metered_channel::Receiver<CheckpointDataToCommit>,
    ) -> Self {
        let checkpoint_commit_batch_size = std::env::var("CHECKPOINT_COMMIT_BATCH_SIZE")
            .unwrap_or(CHECKPOINT_COMMIT_BATCH_SIZE.to_string())
            .parse::<usize>()
            .unwrap();
        info!("Using checkpoint commit batch size {checkpoint_commit_batch_size}");

        let stream =
            ReceiverStream::new(tx_indexing_receiver).ready_chunks(checkpoint_commit_batch_size);

        Self {
            state,
            metrics,
            stream,
            checkpoint_commit_batch_size,
        }
    }

    /// Writes indexed checkpoint data to the database, and then update
    /// watermark upper bounds and metrics. Expects
    /// `indexed_checkpoint_batch` to be non-empty, and contain contiguous
    /// checkpoints. There can be at most one epoch boundary at the end. If
    /// an epoch boundary is detected, epoch-partitioned tables must be
    /// advanced.
    // Unwrap: Caller needs to make sure indexed_checkpoint_batch is not empty
    #[instrument(skip_all, fields(
        first = indexed_checkpoint_batch.first().as_ref().unwrap().checkpoint.sequence_number,
        last = indexed_checkpoint_batch.last().as_ref().unwrap().checkpoint.sequence_number
    ))]
    pub(crate) async fn commit_checkpoints(
        &self,
        indexed_checkpoint_batch: Vec<CheckpointDataToCommit>,
        epoch: Option<EpochToCommit>,
    ) {
        let batch_len = indexed_checkpoint_batch.len();
        let mut checkpoint_batch = Vec::with_capacity(batch_len);
        let mut tx_batch = Vec::with_capacity(batch_len);
        let mut events_batch = Vec::with_capacity(batch_len);
        let mut tx_indices_batch = Vec::with_capacity(batch_len);
        let mut event_indices_batch = Vec::with_capacity(batch_len);
        let mut displays_batch = Vec::with_capacity(batch_len);
        let mut object_changes_batch = Vec::with_capacity(batch_len);
        let mut backward_history_batch = Vec::new();
        let mut object_versions_batch = Vec::with_capacity(batch_len);
        let mut packages_batch = Vec::with_capacity(batch_len);
        let mut account_key_link_ops = Vec::new();
        let mut claimed_accounts = Vec::new();

        for indexed_checkpoint in indexed_checkpoint_batch {
            let CheckpointDataToCommit {
                checkpoint,
                transactions,
                events,
                event_indices,
                tx_indices,
                displays,
                object_changes,
                backward_history_changes,
                object_versions,
                packages,
                account_key_link_ops: checkpoint_link_ops,
                claimed_accounts: checkpoint_claimed_accounts,
                ..
            } = indexed_checkpoint;
            checkpoint_batch.push(checkpoint);
            tx_batch.push(transactions);
            events_batch.push(events);
            tx_indices_batch.push(tx_indices);
            event_indices_batch.push(event_indices);
            displays_batch.extend(displays.into_values());
            object_changes_batch.push(object_changes);
            backward_history_batch.extend(backward_history_changes);
            object_versions_batch.push(object_versions);
            packages_batch.push(packages);
            account_key_link_ops.extend(checkpoint_link_ops);
            claimed_accounts.extend(checkpoint_claimed_accounts);
        }

        // Both tables hold the latest state per key, so collapse the batch to
        // one row per key before writing: the result must not depend on how the
        // batch is chunked. Ops arrive in (checkpoint, transaction, event)
        // order, so the last write wins — which is what makes a claim's
        // `attach` and `claim` ops land as a single row sourced `claim`.
        let account_key_links = collapse_last_write_wins(
            account_key_link_ops.iter().map(StoredAccountKeyLink::from),
            |link| (link.key_id.clone(), link.account_id.clone()),
        );
        let claimed_accounts = collapse_last_write_wins(claimed_accounts.into_iter(), |claimed| {
            claimed.account_id.clone()
        });

        let first_checkpoint_seq = checkpoint_batch.first().as_ref().unwrap().sequence_number;
        let committer_watermark = CommitterWatermark::from(checkpoint_batch.last().unwrap());

        let guard = self.metrics.checkpoint_db_commit_latency.start_timer();
        let tx_batch = tx_batch.into_iter().flatten().collect::<Vec<_>>();

        let tx_global_order_batch: Vec<_> = tx_batch.iter().map(Into::into).collect();
        let tx_indices_batch = tx_indices_batch.into_iter().flatten().collect::<Vec<_>>();
        let events_batch = events_batch.into_iter().flatten().collect::<Vec<_>>();
        let event_indices_batch = event_indices_batch
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        let object_versions_batch = object_versions_batch
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        let packages_batch = packages_batch.into_iter().flatten().collect::<Vec<_>>();
        let checkpoint_num = checkpoint_batch.len();
        let tx_count = tx_batch.len();

        {
            let _step_1_guard = self
                .metrics
                .checkpoint_db_commit_latency_step_1
                .start_timer();
            let mut persist_tasks = vec![
                self.state.persist_transactions(tx_batch),
                self.state.persist_tx_indices(tx_indices_batch),
                self.state.persist_events(events_batch),
                self.state.persist_account_key_links(account_key_links),
                self.state.persist_claimed_accounts(claimed_accounts),
                self.state.persist_event_indices(event_indices_batch),
                self.state.persist_displays(displays_batch),
                self.state
                    .persist_packages(packages_batch.into_iter().map(Into::into).collect()),
                self.state
                    .persist_object_versions(object_versions_batch.clone()),
                Box::pin({
                    let object_changes_batch = object_changes_batch.clone();
                    async move {
                        // We need to persist global order before writing objects, so that
                        // optimistic indexing is blocked from overwriting
                        // objects table with old tx data reference: https://github.com/iotaledger/iota/issues/10250
                        self.state
                            .persist_tx_global_order(tx_global_order_batch.clone())
                            .await?;
                        self.state.persist_objects(object_changes_batch).await
                    }
                }),
                // Backward history must be persisted before checkpointed_objects
                // to prevent read races during consistent view queries.
                Box::pin({
                    let object_changes_batch = object_changes_batch.clone();
                    async move {
                        self.state
                            .persist_object_backward_history(backward_history_batch)
                            .await?;
                        self.state
                            .persist_checkpointed_objects(object_changes_batch)
                            .await
                    }
                }),
            ];
            if let Some(epoch_data) = epoch.clone() {
                persist_tasks.push(self.state.persist_epoch(epoch_data));
            }
            futures::future::join_all(persist_tasks)
                .await
                .into_iter()
                .map(|res| {
                    if res.is_err() {
                        error!("failed to persist data with error: {:?}", res);
                    }
                    res
                })
                .collect::<IndexerResult<Vec<_>>>()
                .expect("persisting data into DB should not fail.");
        }

        let is_epoch_end = epoch.is_some();

        // On epoch boundary, we need to modify the existing partitions' upper bound,
        // and introduce a new partition for incoming data for the upcoming epoch.
        if let Some(epoch_data) = epoch {
            let new_epoch_id = epoch_data.new_epoch.epoch;
            self.state
                .advance_epoch(epoch_data)
                .await
                .tap_err(|e| {
                    error!("failed to advance epoch with error: {}", e.to_string());
                })
                .expect("advancing epochs in DB should not fail.");
            self.metrics.last_committed_epoch.set(new_epoch_id);

            // Refresh participation metrics after advancing epoch
            self.state
                .refresh_participation_metrics()
                .await
                .tap_err(|e| {
                    error!("failed to update participation metrics: {e}");
                })
                .expect("updating participation metrics should not fail.");
        }

        self.state
            .persist_checkpoints(checkpoint_batch)
            .await
            .tap_err(|e| {
                error!(
                    "failed to persist checkpoint data with error: {}",
                    e.to_string()
                );
            })
            .expect("persisting data into DB should not fail.");

        if is_epoch_end {
            // The epoch has advanced so we update the configs for the new protocol version,
            // if it has changed.
            let chain_id = <PgIndexerStore as IndexerStore>::get_chain_identifier(&self.state)
                .await
                .expect("failed to get chain identifier")
                .expect("chain identifier should have been indexed at this point");
            let _ = self
                .state
                .persist_protocol_configs_and_feature_flags(chain_id);
        }

        self.state
            .update_watermarks_upper_bound::<CommitterTables>(committer_watermark)
            .await
            .tap_err(|e| {
                error!(
                    "Failed to update watermark upper bound with error: {}",
                    e.to_string()
                );
            })
            .expect("Updating watermark upper bound in DB should not fail.");

        let elapsed = guard.stop_and_record();

        info!(
            elapsed,
            "Checkpoint {}-{} committed with {} transactions.",
            first_checkpoint_seq,
            committer_watermark.max_committed_cp,
            tx_count,
        );
        self.metrics
            .latest_tx_checkpoint_sequence_number
            .set(committer_watermark.max_committed_cp as i64);
        self.metrics
            .total_tx_checkpoint_committed
            .inc_by(checkpoint_num as u64);
        self.metrics
            .total_transaction_committed
            .inc_by(tx_count as u64);
        self.metrics.transaction_per_checkpoint.observe(
            tx_count as f64
                / (committer_watermark.max_committed_cp - first_checkpoint_seq + 1) as f64,
        );
        // 1000.0 is not necessarily the batch size, it's to roughly map average tx
        // commit latency to [0.1, 1] seconds, which is well covered by
        // DB_COMMIT_LATENCY_SEC_BUCKETS.
        self.metrics
            .thousand_transaction_avg_db_commit_latency
            .observe(elapsed * 1000.0 / tx_count as f64);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        account_key_events::{AccountKeyLinkOp, LinkOpKind, LinkSource},
        models::account_key_links::{LINK_STATUS_ACTIVE, LINK_STATUS_UNLINKED},
    };

    const KEY: [u8; 32] = [0xAA; 32];
    const OTHER_KEY: [u8; 32] = [0xBB; 32];
    const ACCOUNT: [u8; 32] = [0x11; 32];

    #[test]
    fn an_empty_batch_collapses_to_nothing() {
        assert!(collapse(vec![]).is_empty());
    }

    #[test]
    fn the_last_op_per_pair_wins() {
        let rows = collapse(vec![
            op(KEY, LinkSource::Attach, LinkOpKind::Link, 1),
            op(KEY, LinkSource::Detach, LinkOpKind::Unlink, 2),
        ]);

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, LINK_STATUS_UNLINKED);
        assert_eq!(rows[0].last_change_tx_sequence_number, 2);
    }

    #[test]
    fn an_attach_then_claim_in_one_batch_yields_one_claim_row() {
        // The shape of a claim transaction: claim_builder attaches the key and
        // the entry point then emits the claim, both for the same pair.
        let rows = collapse(vec![
            op(KEY, LinkSource::Attach, LinkOpKind::Link, 5),
            op(KEY, LinkSource::Claim, LinkOpKind::Link, 5),
        ]);

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].source, LinkSource::Claim as i16);
        assert_eq!(rows[0].status, LINK_STATUS_ACTIVE);
    }

    #[test]
    fn a_rotation_back_onto_the_same_key_stays_active() {
        let rows = collapse(vec![
            op(KEY, LinkSource::Rotate, LinkOpKind::Unlink, 3),
            op(KEY, LinkSource::Rotate, LinkOpKind::Link, 3),
        ]);

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, LINK_STATUS_ACTIVE);
    }

    #[test]
    fn distinct_keys_keep_distinct_rows_in_key_order() {
        let rows = collapse(vec![
            op(OTHER_KEY, LinkSource::Rotate, LinkOpKind::Link, 4),
            op(KEY, LinkSource::Rotate, LinkOpKind::Unlink, 4),
        ]);

        assert_eq!(rows.len(), 2);
        // Ordered by key so the write set does not depend on batch order.
        assert_eq!(rows[0].key_id, KEY.to_vec());
        assert_eq!(rows[1].key_id, OTHER_KEY.to_vec());
    }

    fn collapse(ops: Vec<AccountKeyLinkOp>) -> Vec<StoredAccountKeyLink> {
        collapse_last_write_wins(ops.iter().map(StoredAccountKeyLink::from), |link| {
            (link.key_id.clone(), link.account_id.clone())
        })
    }

    fn op(
        key_id: [u8; 32],
        source: LinkSource,
        kind: LinkOpKind,
        tx_sequence_number: i64,
    ) -> AccountKeyLinkOp {
        AccountKeyLinkOp {
            key_id,
            account_id: ACCOUNT,
            scheme: 0,
            source,
            kind,
            tx_sequence_number,
            epoch: 1,
        }
    }
}
