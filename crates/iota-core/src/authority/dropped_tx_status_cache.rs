// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! In-memory bounded cache for transactions dropped by white-flag conflict
//! resolution. Allows `wait_for_effects` callers to get an immediate
//! `Rejected` response instead of hanging until timeout.

use std::collections::{HashMap, VecDeque};

use iota_common::sync::notify_read::NotifyRead;
use iota_sdk_types::TransactionDigest;
use iota_types::error::IotaError;
use parking_lot::RwLock;

/// Maximum number of dropped transaction entries to retain.
/// At ~200 bytes per entry (digest + error), 100k entries ≈ 20 MB.
const MAX_DROPPED_ENTRIES: usize = 100_000;

pub(crate) struct DroppedTxStatusCache {
    inner: RwLock<Inner>,
    notify_read: NotifyRead<TransactionDigest, IotaError>,
    /// Told about every drop that `notify_read` is told about, and also about
    /// the drop of one copy of a digest, which is not the digest's status.
    any_drop_notify_read: NotifyRead<TransactionDigest, IotaError>,
}

struct Inner {
    /// Digest → error lookup for the register-then-check pattern.
    entries: HashMap<TransactionDigest, IotaError>,
    /// Insertion order for FIFO eviction when at capacity.
    insertion_order: VecDeque<TransactionDigest>,
}

impl Inner {
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
            insertion_order: VecDeque::new(),
        }
    }

    fn insert(&mut self, digest: TransactionDigest, error: IotaError) {
        // If the digest is already present, skip (don't update ordering).
        if self.entries.contains_key(&digest) {
            return;
        }
        // Evict oldest entries to make room. A digest removed when kept stays
        // in the order, so the order, not the entries, is what is bounded.
        while self.insertion_order.len() >= MAX_DROPPED_ENTRIES {
            if let Some(oldest) = self.insertion_order.pop_front() {
                self.entries.remove(&oldest);
            }
        }
        self.entries.insert(digest, error);
        self.insertion_order.push_back(digest);
    }
}

impl DroppedTxStatusCache {
    pub(crate) fn new() -> Self {
        Self {
            inner: RwLock::new(Inner::new()),
            notify_read: NotifyRead::new(),
            any_drop_notify_read: NotifyRead::new(),
        }
    }

    /// Record a batch of dropped transactions and notify any waiters.
    /// When the cache is at capacity, the oldest entries are evicted first.
    pub(crate) fn insert_and_notify(&self, dropped: &[(TransactionDigest, IotaError)]) {
        {
            let mut inner = self.inner.write();
            for (digest, error) in dropped {
                inner.insert(*digest, error.clone());
            }
        }
        // Notify outside the lock — entries are already visible to readers,
        // so the register-then-check pattern in notify_read_dropped is safe.
        for (digest, error) in dropped {
            self.notify_read.notify(digest, error);
            self.any_drop_notify_read.notify(digest, error);
        }
    }

    /// Notify the current waiters of transactions dropped from one commit
    /// only, without recording the drops: the same digests can still be kept
    /// when sequenced again, so a later request must wait for that outcome.
    pub(crate) fn notify_without_recording(&self, dropped: &[(TransactionDigest, IotaError)]) {
        for (digest, error) in dropped {
            self.notify_read.notify(digest, error);
            self.any_drop_notify_read.notify(digest, error);
        }
    }

    /// Notify only the waiters of [`Self::notify_read_any_drop`] of the drop
    /// of one copy of each digest at a commit. Another copy of the same
    /// digest can still be kept, so the drop is neither recorded nor sent to
    /// the waiters of [`Self::notify_read_dropped`].
    pub(crate) fn notify_dropped_copies(&self, dropped: &[(TransactionDigest, IotaError)]) {
        for (digest, error) in dropped {
            self.any_drop_notify_read.notify(digest, error);
        }
    }

    /// Forget the recorded drops of transactions kept at a commit. The digest
    /// does not cover the signatures, so a drop recorded earlier may be that
    /// of another copy, or of an earlier submission, of a transaction that is
    /// now kept: its status is then the outcome of its execution.
    pub(crate) fn remove_kept(&self, kept: &[TransactionDigest]) {
        let mut inner = self.inner.write();
        for digest in kept {
            inner.entries.remove(digest);
        }
    }

    /// Wait for a transaction to be dropped, or return immediately if it was
    /// already dropped. Uses the register-then-check pattern to avoid the
    /// race where `notify()` fires before the caller registers.
    pub(crate) async fn notify_read_dropped(&self, digest: TransactionDigest) -> IotaError {
        let registration = self.notify_read.register_one(&digest);
        if let Some(error) = self.inner.read().entries.get(&digest) {
            return error.clone();
        }
        registration.await
    }

    /// Like [`Self::notify_read_dropped`], but also returns on the drop of one
    /// copy of the digest at a commit, which is not the digest's status.
    pub(crate) async fn notify_read_any_drop(&self, digest: TransactionDigest) -> IotaError {
        let registration = self.any_drop_notify_read.register_one(&digest);
        if let Some(error) = self.inner.read().entries.get(&digest) {
            return error.clone();
        }
        registration.await
    }

    #[cfg(test)]
    pub(crate) fn num_pending_for_testing(&self) -> usize {
        self.notify_read.num_pending()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::time::timeout;

    use super::*;

    /// When a transaction is already in the cache, `notify_read_dropped`
    /// should return immediately instead of hanging.
    #[tokio::test]
    async fn test_returns_persisted_error() {
        let cache = DroppedTxStatusCache::new();
        let digest = TransactionDigest::random();
        let expected_error = IotaError::TransactionExpired;

        cache.insert_and_notify(&[(digest, expected_error.clone())]);

        let result = timeout(Duration::from_secs(5), cache.notify_read_dropped(digest))
            .await
            .expect("should not timeout");

        assert_eq!(result.to_string(), expected_error.to_string());
    }

    /// The normal path: client registers before the drop is recorded, and
    /// the notification resolves the future.
    #[tokio::test]
    async fn test_waits_for_notification() {
        let cache = std::sync::Arc::new(DroppedTxStatusCache::new());
        let digest = TransactionDigest::random();
        let expected_error = IotaError::TransactionExpired;

        let cache_clone = cache.clone();
        let handle = tokio::spawn(async move { cache_clone.notify_read_dropped(digest).await });

        // Small delay so the spawned task registers before we notify.
        tokio::time::sleep(Duration::from_millis(10)).await;

        cache.insert_and_notify(&[(digest, expected_error.clone())]);

        let result = timeout(Duration::from_secs(5), handle)
            .await
            .expect("should not timeout")
            .expect("task should not panic");

        assert_eq!(result.to_string(), expected_error.to_string());
    }

    /// A drop recorded before the same digest is kept is no longer answered
    /// once it is kept.
    #[tokio::test]
    async fn test_keep_removes_an_earlier_drop() {
        let cache = DroppedTxStatusCache::new();
        let digest = TransactionDigest::random();
        cache.insert_and_notify(&[(digest, IotaError::TransactionExpired)]);

        cache.remove_kept(&[digest]);

        assert!(
            timeout(
                Duration::from_millis(100),
                cache.notify_read_dropped(digest)
            )
            .await
            .is_err(),
            "a request after the keep must wait for the kept transaction's outcome"
        );
    }

    /// The drop of one copy of a digest ends a wait for any drop, but is not
    /// sent to a status request nor recorded for a later one.
    #[tokio::test]
    async fn test_dropped_copy_ends_only_a_wait_for_any_drop() {
        let cache = DroppedTxStatusCache::new();
        let digest = TransactionDigest::random();
        let any_drop = cache.notify_read_any_drop(digest);
        let status = cache.notify_read_dropped(digest);
        futures::pin_mut!(any_drop, status);
        assert!(futures::poll!(&mut any_drop).is_pending());
        assert!(futures::poll!(&mut status).is_pending());

        cache.notify_dropped_copies(&[(digest, IotaError::TransactionExpired)]);

        let error = timeout(Duration::from_secs(5), any_drop)
            .await
            .expect("a wait for any drop must end at the drop of a copy");
        assert_eq!(error, IotaError::TransactionExpired);
        assert!(
            timeout(Duration::from_millis(100), status).await.is_err(),
            "a status request must not get the drop of a copy"
        );
        assert!(
            timeout(
                Duration::from_millis(100),
                cache.notify_read_dropped(digest)
            )
            .await
            .is_err(),
            "the drop of a copy must not be recorded"
        );
    }

    /// When the cache is at capacity, the oldest entries are evicted to make
    /// room for new ones.
    #[tokio::test]
    async fn test_capacity_evicts_oldest() {
        let cache = DroppedTxStatusCache::new();

        // Fill the cache to capacity.
        let entries: Vec<_> = (0..MAX_DROPPED_ENTRIES)
            .map(|_| (TransactionDigest::random(), IotaError::TransactionExpired))
            .collect();
        let first_digest = entries[0].0;
        cache.insert_and_notify(&entries);

        assert_eq!(cache.inner.read().entries.len(), MAX_DROPPED_ENTRIES);
        assert!(cache.inner.read().entries.contains_key(&first_digest));

        // Insert one more — the oldest (first) entry should be evicted.
        let new_digest = TransactionDigest::random();
        cache.insert_and_notify(&[(new_digest, IotaError::TransactionExpired)]);

        assert_eq!(cache.inner.read().entries.len(), MAX_DROPPED_ENTRIES);
        assert!(
            !cache.inner.read().entries.contains_key(&first_digest),
            "oldest entry should have been evicted"
        );
        assert!(
            cache.inner.read().entries.contains_key(&new_digest),
            "new entry should be present"
        );
    }
}
