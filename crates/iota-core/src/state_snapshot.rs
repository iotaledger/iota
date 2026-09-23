// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The hand-over from the epoch boundary to the state snapshot writer.
//!
//! A formal snapshot must describe the live object set exactly as the epoch
//! ended with it, but scanning that set takes minutes and the node has an
//! epoch to get on with. The boundary therefore only waits for a database
//! snapshot of the perpetual store to be taken; the scan and the upload happen
//! while the node executes the next epoch.

use std::{sync::Arc, time::Duration};

use iota_types::committee::EpochId;
use tokio::sync::{OwnedSemaphorePermit, Semaphore, mpsc, oneshot};
use tracing::warn;

/// How long the boundary waits for the writer to take its database snapshot
/// before giving this epoch's state snapshot up.
///
/// Reconfiguration holds the execution write lock across this wait, so it
/// cannot be unbounded: a writer wedged on a blocking-pool slot would
/// otherwise stop the node advancing. It only has to cover scheduling the
/// write loop and the few reads it makes before the snapshot, so a wait this
/// long means something is wrong rather than slow.
const DB_SNAPSHOT_TIMEOUT: Duration = Duration::from_secs(30);

/// The epoch boundary's end of the hand-over.
pub struct EpochSnapshotHandle {
    requests: mpsc::Sender<EpochSnapshotRequest>,
    /// A single permit, held for as long as a snapshot is being written. The
    /// boundary takes it before handing an epoch over, so an epoch arriving
    /// while the previous one is still being written is dropped rather than
    /// queued: waiting for a scan that takes minutes would hold up
    /// reconfiguration, and the next epoch's snapshot is as good.
    write_permits: Arc<Semaphore>,
}

impl EpochSnapshotHandle {
    pub fn new(requests: mpsc::Sender<EpochSnapshotRequest>) -> Self {
        Self {
            requests,
            write_permits: Arc::new(Semaphore::new(1)),
        }
    }

    /// Hands `epoch`'s live object set to the writer and returns once the
    /// writer has taken its snapshot of the perpetual store, which is what
    /// pins the state the state snapshot describes.
    ///
    /// Returns without waiting when the writer is busy or gone, and gives up
    /// after [`DB_SNAPSHOT_TIMEOUT`] if the database snapshot never arrives:
    /// a missing state snapshot costs one epoch of history, holding the
    /// boundary costs the network.
    pub async fn hand_over(&self, epoch: EpochId) -> HandOver {
        let Ok(write_permit) = self.write_permits.clone().try_acquire_owned() else {
            warn!(
                epoch,
                "still writing an earlier epoch's state snapshot; skipping this one"
            );
            return HandOver::Skipped;
        };
        let (db_snapshot_taken, db_snapshot_is_taken) = oneshot::channel();
        let request = EpochSnapshotRequest {
            epoch,
            db_snapshot_taken,
            write_permit,
        };
        if let Err(err) = self.requests.try_send(request) {
            warn!(epoch, "not writing a state snapshot for this epoch: {err}");
            return HandOver::Skipped;
        }
        match tokio::time::timeout(DB_SNAPSHOT_TIMEOUT, db_snapshot_is_taken).await {
            Ok(Ok(())) => HandOver::Started,
            Ok(Err(_)) => {
                warn!(
                    epoch,
                    "the state snapshot writer stopped before taking its database snapshot"
                );
                HandOver::Skipped
            }
            // Dropping the receiver here is what tells the writer to give the
            // epoch up: its send fails, and it abandons the scan rather than
            // reading a store the next epoch is already writing to. A send
            // landing in the instant between this timeout and the drop lets
            // one scan run that the boundary no longer waits for; it is
            // microseconds late, and a scan that late enough to be wrong
            // fails its digest check rather than publishing.
            Err(_) => {
                warn!(
                    epoch,
                    "the state snapshot writer did not take its database snapshot within \
                     {DB_SNAPSHOT_TIMEOUT:?}; giving this epoch's snapshot up"
                );
                HandOver::Skipped
            }
        }
    }
}

/// What became of an epoch offered to the state snapshot writer.
///
/// This node offers an epoch once, when it executes its boundary, so
/// `Skipped` means this node will never publish that epoch's snapshot. A node
/// that has not yet executed the boundary still can.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandOver {
    /// The writer holds a database snapshot of the epoch and is writing its
    /// state snapshot.
    Started,
    /// The epoch gets no snapshot.
    Skipped,
}

/// Asks for the state snapshot of `epoch` to be written.
///
/// Sent while execution is paused at the epoch boundary, so that the database
/// snapshot the writer takes still holds the state the epoch ended with.
pub struct EpochSnapshotRequest {
    /// The epoch whose live object set is to be captured.
    pub epoch: EpochId,
    /// Signalled once the database snapshot exists. The boundary waits for
    /// this and for nothing after it: the snapshot is cheap, the scan behind
    /// it is not.
    ///
    /// Dropped without a signal if the snapshot could not be started, which
    /// leaves the epoch without a snapshot but must not hold the node up.
    ///
    /// A failed send means the boundary has stopped waiting and execution has
    /// resumed, so the writer must abandon the epoch rather than scan a store
    /// that is moving underneath it.
    pub db_snapshot_taken: oneshot::Sender<()>,
    /// Released when the writer is done with this epoch, which is what lets
    /// the next boundary hand one over. See [`EpochSnapshotHandle`].
    pub write_permit: OwnedSemaphorePermit,
}

#[cfg(test)]
mod tests {
    use futures::poll;

    use super::*;

    /// A writer that never takes its database snapshot must not hold
    /// reconfiguration, which runs this under the execution write lock.
    #[tokio::test(start_paused = true)]
    async fn a_boundary_gives_up_on_a_writer_that_never_takes_its_db_snapshot() {
        let (requests, mut received) = mpsc::channel(1);
        let handle = EpochSnapshotHandle::new(requests);

        let handing_over = handle.hand_over(0);
        tokio::pin!(handing_over);
        assert!(
            poll!(&mut handing_over).is_pending(),
            "the boundary waits while the snapshot could still arrive",
        );
        // Held, never signalled: the writer is wedged before its snapshot.
        let request = received.try_recv().expect("the epoch was handed over");

        // `start_paused` advances the clock as soon as nothing is runnable, so
        // this returns without spending the timeout in real time.
        assert_eq!(handing_over.await, HandOver::Skipped);

        assert!(
            request.db_snapshot_taken.is_closed(),
            "giving up must drop the receiver, which is what tells the writer \
             to abandon the epoch rather than scan a moving store",
        );
    }

    /// The boundary waits for the writer's database snapshot, and for nothing
    /// else: an epoch arriving while an earlier one is still being written
    /// gives up its snapshot rather than holding reconfiguration up behind a
    /// scan.
    #[tokio::test]
    async fn a_boundary_does_not_wait_for_a_busy_writer() {
        let (requests, mut received) = mpsc::channel(1);
        let handle = EpochSnapshotHandle::new(requests);

        let first = handle.hand_over(0);
        tokio::pin!(first);
        assert!(
            poll!(&mut first).is_pending(),
            "the boundary must wait until the writer has taken its snapshot",
        );
        let request = received.try_recv().expect("the epoch was handed over");

        // The writer still holds that epoch, so the next one is dropped.
        assert_eq!(
            handle.hand_over(1).await,
            HandOver::Skipped,
            "a busy writer must not hold the boundary up",
        );
        assert!(
            received.try_recv().is_err(),
            "the skipped epoch must not be queued behind the one being written",
        );

        // Taking the snapshot releases the first boundary.
        request
            .db_snapshot_taken
            .send(())
            .expect("the boundary is waiting");
        assert_eq!(
            first.await,
            HandOver::Started,
            "taking the snapshot must release the boundary",
        );

        // Once the writer is done with that epoch, the next one is handed
        // over again, and its boundary waits for its own snapshot.
        drop(request.write_permit);
        let third = handle.hand_over(2);
        tokio::pin!(third);
        assert!(
            poll!(&mut third).is_pending(),
            "the boundary waits for the snapshot of its own epoch",
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
