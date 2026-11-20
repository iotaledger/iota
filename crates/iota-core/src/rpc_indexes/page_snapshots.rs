// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The snapshots of the RPC index store that paginated queries read their
//! pages from, so that the pages of one query do not change while the node
//! keeps indexing checkpoints.
//!
//! The first page of a query reads from a snapshot of the latest indexed
//! checkpoint, which every query starting at that checkpoint shares, so
//! clients cannot make the node take more than one snapshot per indexed
//! checkpoint. A snapshot is dropped once no page followed by another one has
//! read it for the idle timeout, and at the latest once its maximum lifetime
//! has passed, however often it is used. Between pages only the snapshot is
//! kept, never an iterator, so no SST file or dropped history bucket is held.

use std::{
    collections::BTreeMap,
    sync::{Arc, Weak},
    time::Duration,
};

use iota_config::node::PageSnapshotConfig;
use iota_types::{messages_checkpoint::CheckpointSequenceNumber, storage::PageSnapshotRef};
use parking_lot::Mutex;
use prometheus_filtered::{
    Histogram, IntCounterVec, IntGauge, MetricLevel, Registry, register_histogram_with_registry,
    register_int_counter_vec_with_registry, register_int_gauge_with_registry,
};
use tokio::time::Instant;
use typed_store::{SharedDbSnapshot, TypedStoreError, database::Database, rocks::DBMap};

/// The shortest period at which expired snapshots are dropped.
const MIN_SWEEP_PERIOD: Duration = Duration::from_secs(1);

/// The snapshot one page of a paginated query reads from.
pub(crate) struct PageSnapshot {
    pub(crate) snapshot: Arc<SharedDbSnapshot>,
    /// What the token of the next page names, `None` when the snapshot is
    /// not kept for it.
    pub(crate) reference: Option<PageSnapshotRef>,
}

pub(crate) struct PageSnapshots {
    db: Arc<Database>,
    /// Random per open, so that a token from another node or an earlier run
    /// never names one of this store's snapshots.
    node: u64,
    idle_timeout: Duration,
    max_lifetime: Duration,
    max_snapshots: usize,
    kept: Mutex<BTreeMap<CheckpointSequenceNumber, KeptSnapshot>>,
    metrics: PageSnapshotMetrics,
}

struct KeptSnapshot {
    snapshot: Arc<SharedDbSnapshot>,
    taken: Instant,
    extended: Instant,
}

impl PageSnapshots {
    /// Drops expired snapshots in the background while the returned value is
    /// alive, if there is a Tokio runtime to do it on.
    pub(crate) fn new(
        db: Arc<Database>,
        config: &PageSnapshotConfig,
        registry: &Registry,
    ) -> Arc<Self> {
        let snapshots = Arc::new(Self {
            db,
            node: rand::random(),
            idle_timeout: config.idle_timeout(),
            max_lifetime: config.max_lifetime(),
            max_snapshots: config.max_snapshots,
            kept: Mutex::new(BTreeMap::new()),
            metrics: PageSnapshotMetrics::new(registry),
        });
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            let period = (snapshots.idle_timeout / 2).max(MIN_SWEEP_PERIOD);
            let weak = Arc::downgrade(&snapshots);
            runtime.spawn(Self::sweep_periodically(weak, period));
        }
        snapshots
    }

    async fn sweep_periodically(snapshots: Weak<Self>, period: Duration) {
        let mut interval = tokio::time::interval(period);
        loop {
            interval.tick().await;
            let Some(snapshots) = snapshots.upgrade() else {
                break;
            };
            snapshots.drop_expired(Instant::now());
        }
    }

    /// The snapshot `requested` names if it is still kept, otherwise a new
    /// snapshot of the latest checkpoint indexed in `watermark`'s database.
    pub(crate) fn acquire(
        &self,
        requested: Option<PageSnapshotRef>,
        watermark: &DBMap<(), CheckpointSequenceNumber>,
    ) -> Result<PageSnapshot, TypedStoreError> {
        let now = Instant::now();
        if let Some(requested) = requested {
            if let Some(kept) = self.kept_snapshot(requested, now) {
                self.metrics.requests.with_label_values(&["reused"]).inc();
                return Ok(PageSnapshot {
                    snapshot: kept,
                    reference: Some(requested),
                });
            }
            self.metrics.requests.with_label_values(&["expired"]).inc();
        }

        let snapshot = Arc::new(self.db.shared_snapshot());
        // Every checkpoint's index rows are written in one batch together
        // with the watermark, so the snapshot holds exactly the checkpoints
        // up to the watermark it reads.
        let Some(checkpoint) = watermark.get_at_shared_snapshot(&snapshot, &())? else {
            return Ok(PageSnapshot {
                snapshot,
                reference: None,
            });
        };
        let reference = PageSnapshotRef {
            node: self.node,
            checkpoint,
        };

        let mut kept = self.kept.lock();
        if let Some(existing) = kept.get(&checkpoint) {
            self.metrics.requests.with_label_values(&["shared"]).inc();
            return Ok(PageSnapshot {
                snapshot: existing.snapshot.clone(),
                reference: Some(reference),
            });
        }
        if kept.len() >= self.max_snapshots {
            self.drop_expired_locked(&mut kept, now);
            if kept.len() >= self.max_snapshots {
                self.metrics.requests.with_label_values(&["full"]).inc();
                return Ok(PageSnapshot {
                    snapshot,
                    reference: None,
                });
            }
        }
        self.metrics.requests.with_label_values(&["taken"]).inc();
        kept.insert(
            checkpoint,
            KeptSnapshot {
                snapshot: snapshot.clone(),
                taken: now,
                extended: now,
            },
        );
        self.metrics.kept.set(kept.len() as i64);
        Ok(PageSnapshot {
            snapshot,
            reference: Some(reference),
        })
    }

    /// Keeps `reference`'s snapshot for another idle timeout, up to its
    /// maximum lifetime.
    pub(crate) fn extend(&self, reference: PageSnapshotRef) {
        if reference.node != self.node {
            return;
        }
        let now = Instant::now();
        let mut kept = self.kept.lock();
        if let Some(kept) = kept.get_mut(&reference.checkpoint) {
            if now < self.expires_at(kept) {
                kept.extended = now;
            }
        }
    }

    fn kept_snapshot(
        &self,
        reference: PageSnapshotRef,
        now: Instant,
    ) -> Option<Arc<SharedDbSnapshot>> {
        if reference.node != self.node {
            return None;
        }
        let kept = self.kept.lock();
        let kept = kept.get(&reference.checkpoint)?;
        (now < self.expires_at(kept)).then(|| kept.snapshot.clone())
    }

    fn expires_at(&self, kept: &KeptSnapshot) -> Instant {
        (kept.extended + self.idle_timeout).min(kept.taken + self.max_lifetime)
    }

    fn drop_expired(&self, now: Instant) {
        self.drop_expired_locked(&mut self.kept.lock(), now);
    }

    /// Only stops keeping the snapshots: a page still reading one holds it
    /// until it is done.
    fn drop_expired_locked(
        &self,
        kept: &mut BTreeMap<CheckpointSequenceNumber, KeptSnapshot>,
        now: Instant,
    ) {
        kept.retain(|_, snapshot| {
            let keep = now < self.expires_at(snapshot);
            if !keep {
                self.metrics
                    .used_for
                    .observe((snapshot.extended - snapshot.taken).as_secs_f64());
            }
            keep
        });
        self.metrics.kept.set(kept.len() as i64);
    }

    #[cfg(test)]
    pub(crate) fn kept_checkpoints(&self) -> Vec<CheckpointSequenceNumber> {
        self.kept.lock().keys().copied().collect()
    }
}

struct PageSnapshotMetrics {
    kept: IntGauge,
    requests: IntCounterVec,
    used_for: Histogram,
}

impl PageSnapshotMetrics {
    fn new(registry: &Registry) -> Self {
        Self {
            kept: register_int_gauge_with_registry!(
                "rpc_index_page_snapshots",
                "Snapshots of the RPC index store kept for paginated queries",
                registry;
                MetricLevel::Info,
            )
            .unwrap(),
            requests: register_int_counter_vec_with_registry!(
                "rpc_index_page_snapshot_requests",
                "Pages of paginated queries by the snapshot they read from: `reused` the one \
                 their token names, `expired` a new one because that one is gone, `shared` \
                 one another query took at the same checkpoint, `taken` a new one, `full` the \
                 current state because the node keeps as many snapshots as it may",
                &["result"],
                registry;
                MetricLevel::Info,
            )
            .unwrap(),
            used_for: register_histogram_with_registry!(
                "rpc_index_page_snapshot_used_seconds",
                "How long after it was taken a dropped page snapshot was last extended",
                vec![0.0, 1.0, 5.0, 10.0, 30.0, 60.0, 120.0, 180.0, 240.0, 300.0, 600.0],
                registry;
                MetricLevel::Info,
            )
            .unwrap(),
        }
    }
}

#[cfg(test)]
#[path = "../unit_tests/page_snapshots_tests.rs"]
mod tests;
