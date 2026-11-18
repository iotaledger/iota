// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::VecDeque,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use iota_types::messages_checkpoint::CheckpointSequenceNumber;
use tokio::sync::Notify;

/// The checkpoints from which readers have asked for superseded object versions
/// to stay readable. Bucket expiry keeps the buckets from the epoch of the
/// lowest of them on.
pub struct ObjectRetention {
    /// The highest executed checkpoint recorded by [`Self::record_executed`];
    /// it only increases.
    executed: AtomicU64,
    /// One entry per retained checkpoint, in increasing order, shared by every
    /// guard of that checkpoint. An entry no guard holds any more has a
    /// strong count of 1.
    retained: parking_lot::Mutex<VecDeque<Arc<CheckpointSequenceNumber>>>,
    /// Notified each time a guard is dropped.
    released: Arc<Notify>,
}

impl ObjectRetention {
    /// Starts with `executed` as the highest executed checkpoint and no guard.
    pub(crate) fn new(executed: CheckpointSequenceNumber) -> Self {
        Self {
            executed: AtomicU64::new(executed),
            retained: Default::default(),
            released: Default::default(),
        }
    }

    /// Records `executed_seq` as executed. A new guard starts from the highest
    /// checkpoint recorded here, so call it once a checkpoint's outputs are
    /// readable.
    pub(crate) fn record_executed(&self, executed_seq: CheckpointSequenceNumber) {
        // Release pairs with the Acquire loads: a guard that sees a checkpoint
        // as executed also sees its transactions as executed.
        self.executed.fetch_max(executed_seq, Ordering::Release);
    }

    /// Retains the highest executed checkpoint and every later one until the
    /// returned guard is dropped.
    pub(crate) fn retain(&self) -> ObjectRetentionGuard {
        let mut retained = self.retained.lock();
        // Read under the lock, so that a guard an expiry does not see was
        // queued after the expiry looked, and so starts at a checkpoint of an
        // epoch the expiry keeps: expiry runs once every checkpoint of the
        // epochs it drops has been executed.
        let executed = self.executed.load(Ordering::Acquire);
        Self::remove_released(&mut retained);
        if let Some(seq) = retained.back().filter(|seq| ***seq == executed) {
            return ObjectRetentionGuard {
                retained: Some(seq.clone()),
                released: self.released.clone(),
            };
        }
        let seq = Arc::new(executed);
        retained.push_back(seq.clone());
        ObjectRetentionGuard {
            retained: Some(seq),
            released: self.released.clone(),
        }
    }

    /// Notified each time a guard is dropped, which may let an expiry drop a
    /// bucket the guard kept. A drop while nobody waits is remembered for the
    /// next wait.
    pub(crate) fn released(&self) -> Arc<Notify> {
        self.released.clone()
    }

    /// The lowest checkpoint a guard still holds, `None` when no guard is
    /// held.
    pub(crate) fn lowest_retained(&self) -> Option<CheckpointSequenceNumber> {
        let mut retained = self.retained.lock();
        Self::remove_released(&mut retained);
        retained.front().map(|seq| **seq)
    }

    /// Removes the leading entries no guard holds.
    fn remove_released(retained: &mut VecDeque<Arc<CheckpointSequenceNumber>>) {
        while retained
            .front()
            .is_some_and(|seq| Arc::strong_count(seq) == 1)
        {
            // Dropping the last reference synchronizes with every guard of the
            // entry that was dropped, so the reads those guards protected
            // happen before any expiry that follows.
            retained.pop_front();
        }
    }
}

/// Keeps the historic-object buckets from a checkpoint's epoch on from
/// expiring, until dropped.
#[must_use]
pub struct ObjectRetentionGuard {
    /// Always `Some` until the guard is dropped.
    retained: Option<Arc<CheckpointSequenceNumber>>,
    released: Arc<Notify>,
}

impl Drop for ObjectRetentionGuard {
    fn drop(&mut self) {
        // Released before notifying, or the woken expiry could still count
        // this guard as held.
        drop(self.retained.take());
        self.released.notify_one();
    }
}

#[cfg(test)]
mod tests {
    use super::ObjectRetention;

    /// The lowest retained checkpoint is the lowest one some guard still
    /// holds, whatever order guards are dropped in, and none once no guard is
    /// held.
    #[test]
    fn lowest_retained_follows_held_guards() {
        let object_retention = ObjectRetention::new(0);

        object_retention.record_executed(5);
        let first_at_5 = object_retention.retain();
        let second_at_5 = object_retention.retain();
        object_retention.record_executed(6);
        object_retention.record_executed(7);
        let at_7 = object_retention.retain();
        assert_eq!(object_retention.lowest_retained(), Some(5));

        drop(first_at_5);
        assert_eq!(object_retention.lowest_retained(), Some(5));
        drop(at_7);
        assert_eq!(object_retention.lowest_retained(), Some(5));
        drop(second_at_5);
        assert_eq!(object_retention.lowest_retained(), None);

        object_retention.record_executed(8);
        let at_8 = object_retention.retain();
        object_retention.record_executed(9);
        assert_eq!(object_retention.lowest_retained(), Some(8));
        drop(at_8);
        assert_eq!(object_retention.lowest_retained(), None);
    }

    /// The executed checkpoint a guard starts from never goes back, even if
    /// executed checkpoints are recorded out of order.
    #[test]
    fn recorded_executed_checkpoint_only_increases() {
        let object_retention = ObjectRetention::new(0);
        object_retention.record_executed(9);
        object_retention.record_executed(4);
        let _guard = object_retention.retain();
        object_retention.record_executed(10);
        assert_eq!(object_retention.lowest_retained(), Some(9));
    }
}
