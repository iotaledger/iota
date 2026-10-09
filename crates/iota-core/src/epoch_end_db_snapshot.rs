// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Lets a consumer take a database snapshot of the perpetual store as each
//! epoch ends.
//!
//! The epoch boundary hands the epoch to the consumer and waits only until the
//! consumer has taken its [`DbSnapshot`](typed_store::DbSnapshot); whatever the
//! consumer reads through it afterwards runs while the node executes the next
//! epoch.

use std::{sync::Arc, time::Duration};

use iota_types::committee::EpochId;
use tokio::sync::{OwnedSemaphorePermit, Semaphore, mpsc, oneshot};
use tracing::warn;

/// How long the boundary waits for the consumer to take its database snapshot
/// before skipping the epoch.
///
/// Reconfiguration holds the execution write lock across this wait, so it
/// must be bounded.
pub const DB_SNAPSHOT_TIMEOUT: Duration = Duration::from_secs(30);

/// The epoch boundary's end of the hand-over.
pub struct EpochEndDbSnapshotHandle {
    requests: mpsc::Sender<EpochEndDbSnapshotRequest>,
    /// A single permit, held while the consumer is still busy with an epoch.
    /// An epoch arriving while it is taken is skipped rather than queued.
    permits: Arc<Semaphore>,
}

impl EpochEndDbSnapshotHandle {
    pub fn new(requests: mpsc::Sender<EpochEndDbSnapshotRequest>) -> Self {
        Self {
            requests,
            permits: Arc::new(Semaphore::new(1)),
        }
    }

    /// Hands `epoch` to the consumer and returns once the consumer has taken
    /// its database snapshot of the perpetual store.
    ///
    /// Returns [`HandOver::Skipped`] without waiting when the consumer is busy
    /// or gone, and after [`DB_SNAPSHOT_TIMEOUT`] if the database snapshot is
    /// never taken.
    pub async fn hand_over(&self, epoch: EpochId) -> HandOver {
        let Ok(permit) = self.permits.clone().try_acquire_owned() else {
            warn!(
                epoch,
                "the epoch end database snapshot consumer is still busy with an earlier \
                 epoch; skipping this one"
            );
            return HandOver::Skipped;
        };
        let (db_snapshot_taken, db_snapshot_is_taken) = oneshot::channel();
        let request = EpochEndDbSnapshotRequest {
            epoch,
            db_snapshot_taken,
            permit,
        };
        if let Err(err) = self.requests.try_send(request) {
            warn!(
                epoch,
                "not handing this epoch to the epoch end database snapshot consumer: {err}"
            );
            return HandOver::Skipped;
        }
        match tokio::time::timeout(DB_SNAPSHOT_TIMEOUT, db_snapshot_is_taken).await {
            Ok(Ok(())) => HandOver::Started,
            Ok(Err(_)) => {
                warn!(
                    epoch,
                    "the epoch end database snapshot consumer stopped before taking its \
                     database snapshot"
                );
                HandOver::Skipped
            }
            // Dropping the receiver makes the consumer's send fail, which tells
            // it the store is moving again.
            Err(_) => {
                warn!(
                    epoch,
                    "the epoch end database snapshot consumer did not take its database \
                     snapshot within {DB_SNAPSHOT_TIMEOUT:?}; skipping this epoch"
                );
                HandOver::Skipped
            }
        }
    }
}

/// What became of an epoch handed to the consumer.
///
/// This node hands an epoch over once, when it executes its boundary, so
/// after `Skipped` the consumer never gets a database snapshot of that epoch
/// from this node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandOver {
    /// The consumer holds a database snapshot of the state the epoch ended
    /// with.
    Started,
    /// The consumer got no database snapshot of the epoch.
    Skipped,
}

/// Asks the consumer to take a database snapshot of the perpetual store for
/// `epoch`.
///
/// Sent while execution is paused at the epoch boundary, so that the snapshot
/// holds the state the epoch ended with.
pub struct EpochEndDbSnapshotRequest {
    /// The epoch that has just ended.
    pub epoch: EpochId,
    /// Signalled once the database snapshot exists; the boundary waits for
    /// nothing else. Dropped without a signal if the snapshot could not be
    /// taken.
    ///
    /// A failed send means the boundary stopped waiting and execution has
    /// resumed, so the snapshot may include writes from the next epoch. A
    /// successful send means the boundary was still waiting, except for a
    /// send that lands in the instant [`DB_SNAPSHOT_TIMEOUT`] expires.
    pub db_snapshot_taken: oneshot::Sender<()>,
    /// Held until the consumer is done with this epoch. See
    /// [`EpochEndDbSnapshotHandle`].
    pub permit: OwnedSemaphorePermit,
}

#[cfg(test)]
mod tests {
    use futures::poll;

    use super::*;

    /// A consumer that never takes its database snapshot must not hold
    /// reconfiguration, which runs this under the execution write lock.
    #[tokio::test(start_paused = true)]
    async fn a_boundary_gives_up_on_a_consumer_that_never_takes_its_db_snapshot() {
        let (requests, mut received) = mpsc::channel(1);
        let handle = EpochEndDbSnapshotHandle::new(requests);

        let handing_over = handle.hand_over(0);
        tokio::pin!(handing_over);
        assert!(
            poll!(&mut handing_over).is_pending(),
            "the boundary waits while the snapshot could still arrive",
        );
        // Held, never signalled: the consumer is stuck before its snapshot.
        let request = received.try_recv().expect("the epoch was handed over");

        // `start_paused` advances the clock as soon as nothing is runnable, so
        // this returns without spending the timeout in real time.
        assert_eq!(handing_over.await, HandOver::Skipped);

        assert!(
            request.db_snapshot_taken.is_closed(),
            "giving up must drop the receiver, which is what tells the consumer \
             that the store is moving again",
        );
    }

    /// The boundary waits for the consumer's database snapshot and nothing
    /// else, and an epoch arriving while the consumer is still busy with an
    /// earlier one is skipped.
    #[tokio::test]
    async fn a_boundary_does_not_wait_for_a_busy_consumer() {
        let (requests, mut received) = mpsc::channel(1);
        let handle = EpochEndDbSnapshotHandle::new(requests);

        let first = handle.hand_over(0);
        tokio::pin!(first);
        assert!(
            poll!(&mut first).is_pending(),
            "the boundary must wait until the consumer has taken its snapshot",
        );
        let request = received.try_recv().expect("the epoch was handed over");

        // The consumer is still busy with that epoch, so the next one is
        // skipped.
        assert_eq!(
            handle.hand_over(1).await,
            HandOver::Skipped,
            "a busy consumer must not hold the boundary up",
        );
        assert!(
            received.try_recv().is_err(),
            "the skipped epoch must not be queued behind the busy one",
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

        // Once the consumer is done with that epoch, the next one is handed
        // over again, and its boundary waits for its own snapshot.
        drop(request.permit);
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
