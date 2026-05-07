// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Pre-consensus soft locking for the pcool (white-flag) transaction flow.
//!
//! This module provides an in-memory, defense-in-depth mechanism that prevents
//! a validator from accepting two transactions that conflict on the same owned
//! objects at pre-submission time. The authoritative conflict resolution
//! remains in post-consensus validation (`post_consensus_validation.rs`); this
//! layer merely reduces wasted consensus bandwidth and client-visible latency.
//!
//! # Edge cases
//!
//! | Case                            | Behavior                                                              |
//! |---------------------------------|-----------------------------------------------------------------------|
//! | Same tx digest resubmitted      | `try_acquire` is idempotent for same digest — passes through          |
//! | Different tx, same owned objects | Soft lock conflict → `ObjectLockConflict` error                       |
//! | Tx processed by consensus        | Released in `authority_per_epoch_store` after quarantine               |
//! | Tx dropped in post-consensus     | Released in `authority_per_epoch_store` after quarantine               |
//! | Tx forgotten by consensus        | TTL expiry releases locks via background sweep                        |
//! | Consensus submission fails       | Locks released immediately in error path                              |
//! | Crash / restart                  | All soft locks lost → clean slate; post-consensus is authoritative    |
//! | Epoch boundary                   | Same instance is `clear()`-ed; all locks dropped                      |
//! | Object version mismatch          | Keyed on full `ObjectRef`; different versions don't conflict          |
//!
//! # TTL derivation
//!
//! Default TTL = `gc_depth(60) × 4 / ~20 rounds/sec = 12 s`.
//! Must be ≥ 2× the consensus GC window (`gc_depth / round_rate ≈ 3 s`) so that
//! a transaction has time to be committed or garbage-collected before the soft
//! lock expires.  4× provides a safety margin for network jitter.

use std::{
    collections::HashMap,
    sync::{
        Arc, Weak,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use iota_types::{
    base_types::{ObjectRef, TransactionDigest},
    error::IotaError,
};
use parking_lot::Mutex;
use tracing::{debug, error, trace};

use crate::authority_server::metrics::ValidatorServiceMetrics;

/// Default soft-lock TTL: `gc_depth(60) × 4 / ~20 rounds/sec = 12 s`.
const DEFAULT_SOFT_LOCK_TTL: Duration = Duration::from_secs(12);

/// Holder of the owning digest and acquisition time for a single lock.
#[derive(Debug, Clone, Copy)]
struct LockRecord {
    digest: TransactionDigest,
    acquired_at: Instant,
}

/// Mutex-guarded inner state. Holding the two indices behind a single lock
/// makes their cross-consistency a structural invariant rather than a prose
/// claim: `tx_to_objects[d]` and every `locks[obj].digest == d` are always
/// updated atomically in the same critical section.
#[derive(Debug, Default)]
struct Inner {
    /// Maps each owned `ObjectRef` to the record of the transaction that
    /// soft-locked it.
    locks: HashMap<ObjectRef, LockRecord>,
    /// Reverse index: transaction → locked objects, for O(1) batch release.
    tx_to_objects: HashMap<TransactionDigest, Vec<ObjectRef>>,
}

/// In-memory soft locks for pre-consensus owned-object conflict detection.
///
/// Not persisted — crash recovery starts with a clean table.
/// Post-consensus validation is the authoritative conflict resolver.
#[derive(Debug)]
pub struct PreConsensusSoftLocks {
    inner: Mutex<Inner>,
    /// How long a lock remains valid before it is considered expired.
    lock_ttl: Duration,
    /// Atomic mirror of `inner.locks.len()` so metrics readers don't have to
    /// take the mutex. Updated under the inner lock by every mutation, so its
    /// value is always consistent with the map at the moment the lock is
    /// released.
    lock_count: AtomicUsize,
}

impl Default for PreConsensusSoftLocks {
    fn default() -> Self {
        Self::new()
    }
}

impl PreConsensusSoftLocks {
    /// Creates a new instance with the default TTL.
    pub fn new() -> Self {
        Self::with_ttl(DEFAULT_SOFT_LOCK_TTL)
    }

    /// Creates a new instance with a custom TTL (useful for tests).
    pub fn with_ttl(lock_ttl: Duration) -> Self {
        Self {
            inner: Mutex::new(Inner::default()),
            lock_ttl,
            lock_count: AtomicUsize::new(0),
        }
    }

    /// Attempts to soft-lock every `ObjectRef` in `owned_objects` for
    /// `tx_digest`.
    ///
    /// - **Same digest**: idempotent — the lock is refreshed but no error is
    ///   returned. This allows the same transaction to be resubmitted freely.
    /// - **Different digest, unexpired**: returns `ObjectLockConflict`.
    /// - **Expired lock**: silently overwritten by the new transaction.
    ///
    /// On conflict, any locks acquired *within this call* are rolled back (same
    /// pattern as `execution_cache/object_locks.rs`). Both the forward and
    /// reverse indices are updated atomically under a single mutex.
    pub fn try_acquire(
        &self,
        tx_digest: TransactionDigest,
        owned_objects: &[ObjectRef],
    ) -> Result<(), IotaError> {
        if owned_objects.is_empty() {
            return Ok(());
        }

        let now = Instant::now();
        let ttl = self.lock_ttl;
        let mut inner = self.inner.lock();
        let mut acquired: Vec<ObjectRef> = Vec::with_capacity(owned_objects.len());

        for obj_ref in owned_objects {
            match Self::try_set_lock(&mut inner.locks, obj_ref, tx_digest, now, ttl) {
                Ok(()) => acquired.push(*obj_ref),
                Err(e) => {
                    // Rollback locks we just acquired in this call.
                    for rolled in &acquired {
                        if let Some(record) = inner.locks.get(rolled) {
                            if record.digest == tx_digest {
                                inner.locks.remove(rolled);
                            }
                        }
                    }
                    self.lock_count.store(inner.locks.len(), Ordering::Relaxed);
                    return Err(e);
                }
            }
        }

        // Record the reverse mapping so `release()` can find these objects.
        // A digest uniquely determines its owned inputs, so on idempotent
        // resubmission the set is identical. A mismatch indicates a bug; we
        // overwrite so the reverse index stays consistent with `locks`
        // (avoiding split-brain where `release` can't find newly-held refs).
        match inner.tx_to_objects.get(&tx_digest) {
            Some(existing) if existing != &acquired => {
                error!(
                    ?tx_digest,
                    ?existing,
                    ?acquired,
                    "soft-lock reverse index mismatch: \
                     same digest appeared with different owned objects — \
                     overwriting to preserve index consistency"
                );
                debug_assert_eq!(existing, &acquired);
                inner.tx_to_objects.insert(tx_digest, acquired);
            }
            Some(_) => {
                // Matches — nothing to do.
            }
            None => {
                inner.tx_to_objects.insert(tx_digest, acquired);
            }
        }

        self.lock_count.store(inner.locks.len(), Ordering::Relaxed);
        Ok(())
    }

    /// Releases all soft locks held by `tx_digest`.
    ///
    /// Called by active GC hooks (consensus processed / tx dropped) and on
    /// consensus submission failure.
    pub fn release(&self, tx_digest: &TransactionDigest) {
        let mut inner = self.inner.lock();
        Self::release_one(&mut inner, tx_digest);
        self.lock_count.store(inner.locks.len(), Ordering::Relaxed);
    }

    /// Releases all soft locks held by every digest in `tx_digests` under a
    /// single mutex acquisition. Equivalent to calling `release` for each
    /// digest, but avoids `N` lock/unlock cycles per consensus commit.
    pub fn release_for_batch(&self, tx_digests: &[TransactionDigest]) {
        if tx_digests.is_empty() {
            return;
        }
        let mut inner = self.inner.lock();
        for tx_digest in tx_digests {
            Self::release_one(&mut inner, tx_digest);
        }
        self.lock_count.store(inner.locks.len(), Ordering::Relaxed);
    }

    fn release_one(inner: &mut Inner, tx_digest: &TransactionDigest) {
        if let Some(obj_refs) = inner.tx_to_objects.remove(tx_digest) {
            for obj_ref in &obj_refs {
                // Only remove if still owned by this transaction (the lock may
                // have been expired and overwritten by another digest).
                if let Some(record) = inner.locks.get(obj_ref) {
                    if record.digest == *tx_digest {
                        trace!(?tx_digest, ?obj_ref, "soft-lock released");
                        inner.locks.remove(obj_ref);
                    }
                }
            }
        }
    }

    /// Removes all entries whose timestamp is older than `lock_ttl`.
    pub fn sweep_expired(&self) {
        let now = Instant::now();
        let ttl = self.lock_ttl;
        let mut inner = self.inner.lock();

        inner.locks.retain(|_obj_ref, record| {
            let keep = now.duration_since(record.acquired_at) < ttl;
            if !keep {
                debug!(tx_digest = ?record.digest, "soft-lock expired");
            }
            keep
        });

        // Clean up tx_to_objects entries whose locks have all been swept.
        // A digest has a unique set of owned objects, so if none of its
        // objects are still locked under that digest, the entry is stale.
        let Inner {
            locks,
            tx_to_objects,
        } = &mut *inner;
        tx_to_objects.retain(|tx_digest, obj_refs| {
            obj_refs
                .iter()
                .any(|obj_ref| locks.get(obj_ref).is_some_and(|r| r.digest == *tx_digest))
        });

        self.lock_count.store(inner.locks.len(), Ordering::Relaxed);
    }

    /// Returns the current number of locked object refs (for metrics).
    ///
    /// Reads an atomic counter so callers don't have to take the inner mutex —
    /// safe to call from a metrics scrape without contending with acquire /
    /// release on the hot path.
    pub fn lock_count(&self) -> usize {
        self.lock_count.load(Ordering::Relaxed)
    }

    /// Drops all entries.  Called at epoch boundary.
    pub fn clear(&self) {
        let mut inner = self.inner.lock();
        inner.locks.clear();
        inner.tx_to_objects.clear();
        self.lock_count.store(0, Ordering::Relaxed);
    }

    /// Spawns a background task that periodically sweeps expired soft locks
    /// and refreshes the `soft_lock_table_size` gauge, so Prometheus scrapes
    /// see a fresh value even under low transaction load.
    ///
    /// The task holds only a `Weak` reference to the lock table and exits
    /// automatically once all strong `Arc` owners have been dropped (e.g. when
    /// the node stops being a validator). No explicit `abort()` is needed.
    pub fn spawn_sweep(
        soft_locks: Weak<PreConsensusSoftLocks>,
        metrics: Arc<ValidatorServiceMetrics>,
    ) -> tokio::task::JoinHandle<()> {
        iota_metrics::spawn_monitored_task!(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(10));
            loop {
                interval.tick().await;
                let Some(soft_locks) = soft_locks.upgrade() else {
                    // All strong references have been dropped. The intended
                    // cause is that the validator stopped being active (node
                    // shutdown or left the committee). If the node is still
                    // serving, this indicates a leaked `Weak` from a refactor
                    // that accidentally dropped every strong `Arc`, in which
                    // case the `soft_lock_table_size` gauge will freeze and
                    // memory will grow unbounded — hence `warn!` so log-based
                    // alerts can fire.
                    tracing::warn!(
                        "soft-lock sweep task exiting: no strong \
                         `PreConsensusSoftLocks` references remain (expected \
                         during validator shutdown; unexpected otherwise)"
                    );
                    break;
                };
                soft_locks.sweep_expired();
                metrics
                    .soft_lock_table_size
                    .set(soft_locks.lock_count() as i64);
            }
        })
    }

    // -- private helpers -----------------------------------------------------

    /// Test-and-set a single object lock in the forward index.
    ///
    /// Operates on a `&mut HashMap` already borrowed from the outer mutex,
    /// so the caller's critical section is serial over the whole acquire.
    fn try_set_lock(
        locks: &mut HashMap<ObjectRef, LockRecord>,
        obj_ref: &ObjectRef,
        new_digest: TransactionDigest,
        now: Instant,
        lock_ttl: Duration,
    ) -> Result<(), IotaError> {
        let new_record = LockRecord {
            digest: new_digest,
            acquired_at: now,
        };
        match locks.get(obj_ref).copied() {
            None => {
                locks.insert(*obj_ref, new_record);
                Ok(())
            }
            Some(existing) => {
                if existing.digest == new_digest {
                    // Same transaction — refresh timestamp, no conflict.
                    locks.insert(*obj_ref, new_record);
                    return Ok(());
                }

                if now.duration_since(existing.acquired_at) >= lock_ttl {
                    // Expired — overwrite.
                    trace!(
                        ?obj_ref,
                        existing_digest = ?existing.digest,
                        ?new_digest,
                        "soft-lock expired, overwriting"
                    );
                    locks.insert(*obj_ref, new_record);
                    Ok(())
                } else {
                    debug!(
                        ?obj_ref,
                        existing_digest = ?existing.digest,
                        ?new_digest,
                        "soft-lock conflict"
                    );
                    Err(IotaError::ObjectLockConflict {
                        obj_ref: *obj_ref,
                        pending_transaction: existing.digest,
                    })
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use iota_sdk_types::ObjectId;
    use iota_types::base_types::{ObjectDigest, SequenceNumber};

    use super::*;

    fn obj_ref(id: u8, version: u64) -> ObjectRef {
        ObjectRef::new(
            ObjectId::new([id; ObjectId::LENGTH]),
            SequenceNumber::from_u64(version),
            ObjectDigest::random(),
        )
    }

    fn digest(byte: u8) -> TransactionDigest {
        TransactionDigest::new([byte; 32])
    }

    #[test]
    fn test_acquire_and_release() {
        let table = PreConsensusSoftLocks::with_ttl(Duration::from_secs(60));
        let tx = digest(1);
        let objs = vec![obj_ref(1, 1), obj_ref(2, 1)];

        table.try_acquire(tx, &objs).unwrap();
        assert_eq!(table.lock_count(), 2);

        table.release(&tx);
        assert_eq!(table.lock_count(), 0);
    }

    #[test]
    fn test_conflict_different_digest() {
        let table = PreConsensusSoftLocks::with_ttl(Duration::from_secs(60));
        let obj = obj_ref(1, 1);
        let tx_a = digest(1);
        let tx_b = digest(2);

        table.try_acquire(tx_a, &[obj]).unwrap();
        let err = table.try_acquire(tx_b, &[obj]).unwrap_err();
        assert!(matches!(err, IotaError::ObjectLockConflict { .. }));
    }

    #[test]
    fn test_same_digest_idempotent() {
        let table = PreConsensusSoftLocks::with_ttl(Duration::from_secs(60));
        let obj = obj_ref(1, 1);
        let tx = digest(1);

        table.try_acquire(tx, &[obj]).unwrap();
        // Same digest again — should succeed.
        table.try_acquire(tx, &[obj]).unwrap();
        assert_eq!(table.lock_count(), 1);
    }

    #[test]
    fn test_ttl_expiry_allows_reacquisition() {
        // Use a zero TTL so locks expire immediately.
        let table = PreConsensusSoftLocks::with_ttl(Duration::ZERO);
        let obj = obj_ref(1, 1);
        let tx_a = digest(1);
        let tx_b = digest(2);

        table.try_acquire(tx_a, &[obj]).unwrap();
        // tx_b should succeed because tx_a's lock is already expired.
        table.try_acquire(tx_b, &[obj]).unwrap();
    }

    #[test]
    fn test_rollback_on_partial_conflict() {
        let table = PreConsensusSoftLocks::with_ttl(Duration::from_secs(60));
        let obj_x = obj_ref(1, 1);
        let obj_y = obj_ref(2, 1);
        let obj_z = obj_ref(3, 1);
        let tx_a = digest(1);
        let tx_b = digest(2);

        // tx_a locks X and Y.
        table.try_acquire(tx_a, &[obj_x, obj_y]).unwrap();

        // tx_b tries Z then Y — Z is acquired, Y conflicts, so Z must be
        // rolled back.
        let err = table.try_acquire(tx_b, &[obj_z, obj_y]).unwrap_err();
        assert!(matches!(err, IotaError::ObjectLockConflict { .. }));

        // Only 2 locks should exist (tx_a's X and Y), not 3.
        assert_eq!(table.lock_count(), 2);

        // Z should be lockable by a third tx.
        let tx_c = digest(3);
        table.try_acquire(tx_c, &[obj_z]).unwrap();

        // tx_b never fully acquired, so it must have no reverse-index entry
        // (otherwise a later `release(&tx_b)` would wrongly free tx_c's lock).
        assert!(
            !table.inner.lock().tx_to_objects.contains_key(&tx_b),
            "losing tx must not leave a stale reverse-index entry"
        );
    }

    /// The idempotent same-digest path must refresh the lock's timestamp so a
    /// legitimately-retrying client doesn't see its own lock expire mid-flight.
    /// We inspect the stored `Instant` directly rather than race the wall
    /// clock.
    #[test]
    fn test_same_digest_refreshes_timestamp() {
        let table = PreConsensusSoftLocks::with_ttl(Duration::from_secs(60));
        let obj = obj_ref(1, 1);
        let tx = digest(1);

        table.try_acquire(tx, &[obj]).unwrap();
        let first_ts = table.inner.lock().locks.get(&obj).unwrap().acquired_at;

        // Busy-wait until `Instant::now()` strictly advances so the refresh is
        // observable regardless of platform clock resolution.
        while Instant::now() <= first_ts {
            std::hint::spin_loop();
        }

        table.try_acquire(tx, &[obj]).unwrap();
        let second_ts = table.inner.lock().locks.get(&obj).unwrap().acquired_at;

        assert!(
            second_ts > first_ts,
            "same-digest re-acquire must refresh the timestamp (first={first_ts:?}, second={second_ts:?})",
        );
    }

    /// Concurrent `try_acquire` calls for different digests on the same
    /// object must produce exactly one winner and `N-1` `ObjectLockConflict`
    /// errors — no lost locks, no duplicate wins.
    #[test]
    fn test_concurrent_acquire_exactly_one_winner() {
        use std::{
            sync::{
                Arc, Barrier,
                atomic::{AtomicUsize, Ordering},
            },
            thread,
        };

        let table = Arc::new(PreConsensusSoftLocks::with_ttl(Duration::from_secs(60)));
        let obj = obj_ref(1, 1);
        let n = 16;
        let barrier = Arc::new(Barrier::new(n));
        let wins = Arc::new(AtomicUsize::new(0));
        let conflicts = Arc::new(AtomicUsize::new(0));

        let handles: Vec<_> = (0..n)
            .map(|i| {
                let table = table.clone();
                let barrier = barrier.clone();
                let wins = wins.clone();
                let conflicts = conflicts.clone();
                thread::spawn(move || {
                    barrier.wait();
                    match table.try_acquire(digest(i as u8 + 1), &[obj]) {
                        Ok(()) => wins.fetch_add(1, Ordering::Relaxed),
                        Err(IotaError::ObjectLockConflict { .. }) => {
                            conflicts.fetch_add(1, Ordering::Relaxed)
                        }
                        Err(e) => panic!("unexpected error: {e:?}"),
                    };
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(wins.load(Ordering::Relaxed), 1, "exactly one winner");
        assert_eq!(conflicts.load(Ordering::Relaxed), n - 1, "rest conflict");
        assert_eq!(table.lock_count(), 1);
    }

    #[test]
    fn test_sweep_expired() {
        let table = PreConsensusSoftLocks::with_ttl(Duration::ZERO);
        let objs = vec![obj_ref(1, 1), obj_ref(2, 1)];
        let tx = digest(1);

        table.try_acquire(tx, &objs).unwrap();
        assert_eq!(table.lock_count(), 2);

        table.sweep_expired();
        assert_eq!(table.lock_count(), 0);
        // Reverse index should also be cleaned up.
        assert!(!table.inner.lock().tx_to_objects.contains_key(&tx));
    }

    #[test]
    fn test_sweep_cleans_stale_reverse_index_after_overwrite() {
        // Use zero TTL so that tx_a's lock expires immediately and tx_b can
        // overwrite it. The old tx_a entry in tx_to_objects becomes stale.
        let table = PreConsensusSoftLocks::with_ttl(Duration::ZERO);
        let obj = obj_ref(1, 1);
        let tx_a = digest(1);
        let tx_b = digest(2);

        table.try_acquire(tx_a, &[obj]).unwrap();
        // tx_a's lock is already expired (TTL=0), tx_b overwrites it.
        table.try_acquire(tx_b, &[obj]).unwrap();

        // tx_a's tx_to_objects entry is stale (obj now belongs to tx_b).
        assert!(table.inner.lock().tx_to_objects.contains_key(&tx_a));

        // Sweep should clean up both expired locks AND stale reverse index.
        // tx_b's lock is also expired (TTL=0), so everything should be cleaned.
        table.sweep_expired();
        assert_eq!(table.lock_count(), 0);
        let inner = table.inner.lock();
        assert!(!inner.tx_to_objects.contains_key(&tx_a));
        assert!(!inner.tx_to_objects.contains_key(&tx_b));
    }

    #[test]
    fn test_clear() {
        let table = PreConsensusSoftLocks::with_ttl(Duration::from_secs(60));
        let objs = vec![obj_ref(1, 1), obj_ref(2, 1)];
        let tx = digest(1);

        table.try_acquire(tx, &objs).unwrap();
        table.clear();

        assert_eq!(table.lock_count(), 0);
        assert!(table.inner.lock().tx_to_objects.is_empty());
    }

    #[test]
    fn test_empty_owned_objects() {
        let table = PreConsensusSoftLocks::with_ttl(Duration::from_secs(60));
        let tx = digest(1);
        table.try_acquire(tx, &[]).unwrap();
        assert_eq!(table.lock_count(), 0);
    }

    #[test]
    #[should_panic(expected = "assertion `left == right` failed")]
    fn test_same_digest_different_objects_panics() {
        let table = PreConsensusSoftLocks::with_ttl(Duration::from_secs(60));
        let tx = digest(1);
        let obj_a = obj_ref(1, 1);
        let obj_b = obj_ref(2, 1);

        table.try_acquire(tx, &[obj_a]).unwrap();
        // Manually force the same digest with a different object set to
        // trigger the inconsistency detection. In production this cannot
        // happen through `try_acquire` because `try_set_lock` would see
        // the same digest and succeed idempotently with the same objects.
        // We bypass that by inserting directly into `tx_to_objects`.
        table.inner.lock().tx_to_objects.insert(tx, vec![obj_b]);
        // Re-acquire — the mismatch path detects it and `debug_assert_eq!`
        // fires under test builds.
        table.try_acquire(tx, &[obj_a]).unwrap();
    }

    /// Concurrent `release` and `try_acquire` for distinct digests must not
    /// corrupt the table. Regression guard for future refactors that might
    /// split the inner mutex.
    #[test]
    fn test_concurrent_release_and_acquire() {
        use std::{
            sync::{Arc, Barrier},
            thread,
        };

        let table = Arc::new(PreConsensusSoftLocks::with_ttl(Duration::from_secs(60)));
        let n = 16;
        // Pre-populate locks held by digests 1..=n on object refs 1..=n.
        let objs: Vec<_> = (0..n).map(|i| obj_ref(i as u8 + 1, 1)).collect();
        let digests: Vec<_> = (0..n).map(|i| digest(i as u8 + 1)).collect();
        for (d, o) in digests.iter().zip(objs.iter()) {
            table.try_acquire(*d, &[*o]).unwrap();
        }
        assert_eq!(table.lock_count(), n);

        // Half the threads release pre-existing locks; the other half acquire
        // brand-new locks on disjoint object refs. Disjoint key sets mean no
        // conflict is expected — every operation should succeed.
        let barrier = Arc::new(Barrier::new(2 * n));
        let mut handles = Vec::with_capacity(2 * n);

        for &d in &digests {
            let table = table.clone();
            let barrier = barrier.clone();
            handles.push(thread::spawn(move || {
                barrier.wait();
                table.release(&d);
            }));
        }
        for i in 0..n {
            let table = table.clone();
            let barrier = barrier.clone();
            let new_obj = obj_ref(100 + i as u8, 1);
            let new_digest = digest(100 + i as u8);
            handles.push(thread::spawn(move || {
                barrier.wait();
                table.try_acquire(new_digest, &[new_obj]).unwrap();
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        // After all releases + acquires: original n locks gone, n new locks held.
        assert_eq!(table.lock_count(), n);
        let inner = table.inner.lock();
        for d in &digests {
            assert!(
                !inner.tx_to_objects.contains_key(d),
                "released digest must not retain reverse-index entry"
            );
        }
    }

    /// `sweep_expired` must respect a non-zero TTL: locks younger than the
    /// TTL stay, older ones are removed. The existing zero-TTL test cannot
    /// distinguish `>=` from `>` in the comparison.
    #[test]
    fn test_sweep_expired_with_nonzero_ttl() {
        let ttl = Duration::from_millis(50);
        let table = PreConsensusSoftLocks::with_ttl(ttl);
        let old = obj_ref(1, 1);
        let tx_old = digest(1);

        table.try_acquire(tx_old, &[old]).unwrap();

        // Wait past TTL so the first lock is definitely expired.
        std::thread::sleep(ttl * 3);

        // Acquire a fresh lock right before the sweep — this one must survive.
        let fresh = obj_ref(2, 1);
        let tx_fresh = digest(2);
        table.try_acquire(tx_fresh, &[fresh]).unwrap();

        table.sweep_expired();

        let inner = table.inner.lock();
        assert!(
            !inner.locks.contains_key(&old),
            "expired lock must be swept"
        );
        assert!(
            inner.locks.contains_key(&fresh),
            "fresh lock must survive sweep"
        );
        assert!(!inner.tx_to_objects.contains_key(&tx_old));
        assert!(inner.tx_to_objects.contains_key(&tx_fresh));
    }

    #[test]
    fn test_release_for_batch() {
        let table = PreConsensusSoftLocks::with_ttl(Duration::from_secs(60));
        let tx_a = digest(1);
        let tx_b = digest(2);
        let tx_c = digest(3);

        table.try_acquire(tx_a, &[obj_ref(1, 1)]).unwrap();
        table
            .try_acquire(tx_b, &[obj_ref(2, 1), obj_ref(3, 1)])
            .unwrap();
        table.try_acquire(tx_c, &[obj_ref(4, 1)]).unwrap();
        assert_eq!(table.lock_count(), 4);

        table.release_for_batch(&[tx_a, tx_b]);

        assert_eq!(table.lock_count(), 1);
        let inner = table.inner.lock();
        assert!(!inner.tx_to_objects.contains_key(&tx_a));
        assert!(!inner.tx_to_objects.contains_key(&tx_b));
        assert!(inner.tx_to_objects.contains_key(&tx_c));
    }

    #[test]
    fn test_release_for_batch_empty_is_noop() {
        let table = PreConsensusSoftLocks::with_ttl(Duration::from_secs(60));
        let tx = digest(1);
        table.try_acquire(tx, &[obj_ref(1, 1)]).unwrap();
        table.release_for_batch(&[]);
        assert_eq!(table.lock_count(), 1);
    }

    #[test]
    fn test_release_nonexistent_is_noop() {
        let table = PreConsensusSoftLocks::with_ttl(Duration::from_secs(60));
        // Should not panic.
        table.release(&digest(42));
    }
}
