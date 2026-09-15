// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The hand-over from the epoch boundary to the state snapshot writer.
//!
//! A formal snapshot must describe the live object set exactly as the epoch
//! ended with it, but scanning that set takes minutes and the node has an
//! epoch to get on with. The boundary therefore only takes a read view of the
//! perpetual store and hands it over; the scan and the upload happen while the
//! node executes the next epoch.

use std::sync::Arc;

use iota_types::committee::EpochId;
use tokio::sync::{OwnedSemaphorePermit, Semaphore, mpsc, oneshot};
use tracing::warn;

use crate::authority::authority_store_tables::AuthorityPerpetualTables;

/// The epoch boundary's end of the hand-over.
pub struct EpochSnapshotHandle {
    requests: mpsc::Sender<EpochSnapshotRequest>,
    /// A single permit, held for as long as a snapshot is being written. The
    /// boundary takes it before handing an epoch over, so an epoch arriving
    /// while the previous one is still being written is dropped rather than
    /// queued: waiting for a scan that takes minutes would hold up
    /// reconfiguration, and the next epoch's snapshot is as good.
    writer_idle: Arc<Semaphore>,
}

impl EpochSnapshotHandle {
    pub fn new(requests: mpsc::Sender<EpochSnapshotRequest>) -> Self {
        Self {
            requests,
            writer_idle: Arc::new(Semaphore::new(1)),
        }
    }

    /// Hands `epoch`'s live object set to the writer and returns once the
    /// writer has taken its read view of `perpetual_tables`, which is what
    /// pins the state the snapshot describes.
    ///
    /// Returns without waiting when the writer is busy or gone: a missing
    /// snapshot costs one epoch of history, holding the boundary costs the
    /// network.
    pub async fn hand_over(&self, epoch: EpochId, perpetual_tables: Arc<AuthorityPerpetualTables>) {
        let Ok(writer_idle) = self.writer_idle.clone().try_acquire_owned() else {
            warn!(
                epoch,
                "still writing an earlier epoch's state snapshot; skipping this one"
            );
            return;
        };
        let (view_taken, view_is_taken) = oneshot::channel();
        let request = EpochSnapshotRequest {
            epoch,
            perpetual_tables,
            view_taken,
            writer_idle,
        };
        if let Err(err) = self.requests.try_send(request) {
            warn!(epoch, "not writing a state snapshot for this epoch: {err}");
            return;
        }
        if view_is_taken.await.is_err() {
            warn!(
                epoch,
                "the state snapshot writer stopped before taking its read view"
            );
        }
    }
}

/// Asks for the state snapshot of `epoch` to be written.
///
/// Sent while execution is paused at the epoch boundary, so that the read view
/// the writer takes still holds the state the epoch ended with.
pub struct EpochSnapshotRequest {
    /// The epoch whose live object set is to be captured.
    pub epoch: EpochId,
    /// The store the writer takes its read view of.
    pub perpetual_tables: Arc<AuthorityPerpetualTables>,
    /// Signalled once the read view exists. The boundary waits for this and
    /// for nothing after it: a view is cheap, the scan behind it is not.
    ///
    /// Dropped without a signal if the snapshot could not be started, which
    /// leaves the epoch without a snapshot but must not hold the node up.
    pub view_taken: oneshot::Sender<()>,
    /// Released when the writer is done with this epoch, which is what lets
    /// the next boundary hand one over. See [`EpochSnapshotHandle`].
    pub writer_idle: OwnedSemaphorePermit,
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::time::timeout;

    use super::*;

    fn perpetual_tables(dir: &tempfile::TempDir) -> Arc<AuthorityPerpetualTables> {
        Arc::new(AuthorityPerpetualTables::open(dir.path(), None))
    }

    /// The boundary waits for the writer's read view, and for nothing else: an
    /// epoch arriving while an earlier one is still being written gives up its
    /// snapshot rather than holding reconfiguration up behind a scan.
    #[tokio::test]
    async fn a_boundary_does_not_wait_for_a_busy_writer() {
        let dir = iota_common::tempdir();
        let tables = perpetual_tables(&dir);
        let (requests, mut received) = mpsc::channel(1);
        let handle = EpochSnapshotHandle::new(requests);

        let first = handle.hand_over(0, tables.clone());
        tokio::pin!(first);
        assert!(
            timeout(Duration::from_millis(50), &mut first)
                .await
                .is_err(),
            "the boundary must wait until the writer has taken its read view",
        );
        let request = received.try_recv().expect("the epoch was handed over");

        // The writer still holds that epoch, so the next one is dropped.
        timeout(
            Duration::from_millis(50),
            handle.hand_over(1, tables.clone()),
        )
        .await
        .expect("a busy writer must not hold the boundary up");
        assert!(
            received.try_recv().is_err(),
            "the skipped epoch must not be queued behind the one being written",
        );

        // Taking the view releases the first boundary.
        request
            .view_taken
            .send(())
            .expect("the boundary is waiting");
        timeout(Duration::from_millis(50), first)
            .await
            .expect("taking the read view must release the boundary");

        // Once the writer is done with that epoch, the next one is handed
        // over again, and its boundary waits for its own read view.
        drop(request.writer_idle);
        let third = handle.hand_over(2, tables);
        tokio::pin!(third);
        assert!(
            timeout(Duration::from_millis(50), &mut third)
                .await
                .is_err(),
            "the boundary waits for the read view of its own epoch",
        );
        assert_eq!(
            received
                .try_recv()
                .expect("the epoch was handed over")
                .epoch,
            2,
        );
    }
}
