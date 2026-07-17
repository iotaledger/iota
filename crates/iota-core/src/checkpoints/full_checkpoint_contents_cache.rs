// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use iota_config::node::DEFAULT_FULL_CHECKPOINT_CONTENTS_CACHE_SIZE_MB;
use iota_sdk_types::CheckpointContentsDigest;
use iota_types::messages_checkpoint::{CheckpointSequenceNumber, FullCheckpointContents};
use parking_lot::Mutex;
use prometheus_filtered::{
    IntCounter, IntGauge, Registry, register_int_counter_vec_with_registry,
    register_int_counter_with_registry, register_int_gauge_with_registry,
};

/// Value of the `lookup` metrics label for sequence-number lookups: the
/// checkpoint executor's bulk load and `ReadStore` reads by sequence number.
const LOOKUP_BY_SEQ: &str = "by_seq";
/// Value of the `lookup` metrics label for contents-digest lookups: serving
/// checkpoint-contents requests from state-sync peers.
const LOOKUP_BY_DIGEST: &str = "by_digest";

/// The hit/miss counters are exported as `IntCounterVec`s with a `lookup`
/// label; the per-label children are resolved once at construction so the
/// lookup paths increment plain counters.
pub struct FullCheckpointContentsCacheMetrics {
    /// Sequence-number lookups served from the cache (`lookup="by_seq"`).
    pub hits_by_seq: IntCounter,
    /// Contents-digest lookups served from the cache (`lookup="by_digest"`).
    pub hits_by_digest: IntCounter,
    /// Sequence-number lookups that missed the cache (`lookup="by_seq"`).
    ///
    /// On validators these accrue once per self-produced checkpoint by
    /// design: the executor checks the cache before inserting the entry it
    /// then produces.
    pub misses_by_seq: IntCounter,
    /// Contents-digest lookups that missed the cache (`lookup="by_digest"`)
    /// — the health signal for whether peers are served from memory or fall
    /// back to reconstruction.
    pub misses_by_digest: IntCounter,
    pub evictions: IntCounter,
    pub entries: IntGauge,
    pub total_bytes: IntGauge,
}

impl FullCheckpointContentsCacheMetrics {
    pub fn new(registry: &Registry) -> Arc<Self> {
        let hits = register_int_counter_vec_with_registry!(
            "full_checkpoint_contents_cache_hits",
            "Number of full-checkpoint-contents cache lookups served from the cache, \
            partitioned by lookup kind",
            &["lookup"],
            registry
        )
        .unwrap();
        let misses = register_int_counter_vec_with_registry!(
            "full_checkpoint_contents_cache_misses",
            "Number of full-checkpoint-contents cache lookups that missed the cache, \
            partitioned by lookup kind",
            &["lookup"],
            registry
        )
        .unwrap();
        Arc::new(Self {
            hits_by_seq: hits.with_label_values(&[LOOKUP_BY_SEQ]),
            hits_by_digest: hits.with_label_values(&[LOOKUP_BY_DIGEST]),
            misses_by_seq: misses.with_label_values(&[LOOKUP_BY_SEQ]),
            misses_by_digest: misses.with_label_values(&[LOOKUP_BY_DIGEST]),
            evictions: register_int_counter_with_registry!(
                "full_checkpoint_contents_cache_evictions",
                "Number of entries evicted from the full-checkpoint-contents cache",
                registry
            )
            .unwrap(),
            entries: register_int_gauge_with_registry!(
                "full_checkpoint_contents_cache_entries",
                "Number of entries currently held in the full-checkpoint-contents cache",
                registry
            )
            .unwrap(),
            total_bytes: register_int_gauge_with_registry!(
                "full_checkpoint_contents_cache_bytes",
                "Approximate serialized size of the full-checkpoint-contents cache in bytes",
                registry
            )
            .unwrap(),
        })
    }

    pub fn new_for_tests() -> Arc<Self> {
        Self::new(&Registry::new())
    }
}

struct CacheEntry {
    digest: CheckpointContentsDigest,
    contents: Arc<FullCheckpointContents>,
    bytes: usize,
}

#[derive(Default)]
struct Inner {
    by_seq: BTreeMap<CheckpointSequenceNumber, CacheEntry>,
    seq_by_digest: HashMap<CheckpointContentsDigest, CheckpointSequenceNumber>,
    total_bytes: usize,
}

impl Inner {
    /// Removes the digest mapping of an entry leaving `by_seq`, unless the
    /// mapping points to a different sequence number: distinct checkpoints
    /// can share a contents digest (e.g. consecutive empty checkpoints), and
    /// the mapping must keep serving the entry that is still cached.
    fn remove_digest_mapping(
        &mut self,
        digest: &CheckpointContentsDigest,
        seq: CheckpointSequenceNumber,
    ) {
        if self.seq_by_digest.get(digest) == Some(&seq) {
            self.seq_by_digest.remove(digest);
        }
    }
}

/// A size-bounded in-memory cache of [`FullCheckpointContents`], keyed by
/// checkpoint sequence number with a secondary contents-digest index.
///
/// This replaces the former `full_checkpoint_content` RocksDB table as the
/// fast path for the checkpoint executor's bulk transaction load and for
/// serving checkpoint contents to state-sync peers. All consumers have
/// fallbacks that reconstruct the contents from the permanent stores, so
/// entries can be evicted (or the cache disabled with a zero budget) without
/// affecting correctness.
///
/// When the byte budget is exceeded, entries with the lowest sequence numbers
/// are evicted first, keeping the newest window of checkpoints — the ones
/// tip-following peers request. Inserts older than the window of a full cache
/// are skipped, since eviction would remove them immediately. A single entry
/// larger than the whole budget is still cached until the next insert
/// displaces it. A budget of zero disables the cache entirely.
///
/// The budget is accounted in serialized (BCS) bytes — the resident heap
/// footprint of a full cache is somewhat higher than the budget.
pub struct FullCheckpointContentsCache {
    max_bytes: usize,
    metrics: Arc<FullCheckpointContentsCacheMetrics>,
    inner: Mutex<Inner>,
}

impl Default for FullCheckpointContentsCache {
    /// A cache with the default byte budget and unregistered metrics, for
    /// contexts without a node config or metrics registry (tools, tests).
    fn default() -> Self {
        Self::new(
            DEFAULT_FULL_CHECKPOINT_CONTENTS_CACHE_SIZE_MB * 1024 * 1024,
            FullCheckpointContentsCacheMetrics::new_for_tests(),
        )
    }
}

impl FullCheckpointContentsCache {
    pub fn new(max_bytes: usize, metrics: Arc<FullCheckpointContentsCacheMetrics>) -> Self {
        Self {
            max_bytes,
            metrics,
            inner: Mutex::new(Inner::default()),
        }
    }

    /// Whether contents for `seq` are worth assembling and inserting: false
    /// when the cache is disabled, and false when `seq` is older than
    /// everything cached — lowest-seq eviction would drop the entry again.
    ///
    /// Lets callers skip the expensive contents assembly that precedes
    /// [`Self::insert`], e.g. the checkpoint executor during deep catch-up,
    /// where the cache window rides the state-sync frontier far ahead of the
    /// executor.
    pub fn should_cache(&self, seq: CheckpointSequenceNumber) -> bool {
        if self.max_bytes == 0 {
            return false;
        }

        let inner = self.inner.lock();
        inner
            .by_seq
            .first_key_value()
            .is_none_or(|(lowest, _)| seq >= *lowest)
    }

    /// Inserts the contents for a checkpoint and evicts the lowest sequence
    /// numbers until the cache fits its byte budget again.
    pub fn insert(
        &self,
        seq: CheckpointSequenceNumber,
        digest: CheckpointContentsDigest,
        contents: Arc<FullCheckpointContents>,
        bytes: usize,
    ) {
        if self.max_bytes == 0 {
            return;
        }
        let mut inner = self.inner.lock();

        // An entry that would push the cache over budget while being older
        // than everything cached gets removed first by lowest-seq eviction —
        // skip the pointless insert.
        if inner.total_bytes + bytes > self.max_bytes
            && inner
                .by_seq
                .first_key_value()
                .is_some_and(|(lowest, _)| seq < *lowest)
        {
            return;
        }

        // Hold displaced entries so their (potentially multi-MB) contents are
        // freed after the lock is released, not under it.
        let mut displaced = Vec::new();

        if let Some(old) = inner.by_seq.insert(
            seq,
            CacheEntry {
                digest,
                contents,
                bytes,
            },
        ) {
            inner.total_bytes -= old.bytes;
            inner.remove_digest_mapping(&old.digest, seq);
            displaced.push(old.contents);
        }
        inner.total_bytes += bytes;
        inner.seq_by_digest.insert(digest, seq);

        while inner.total_bytes > self.max_bytes && inner.by_seq.len() > 1 {
            let (evicted_seq, evicted) = inner.by_seq.pop_first().expect("cache is not empty");
            inner.total_bytes -= evicted.bytes;
            inner.remove_digest_mapping(&evicted.digest, evicted_seq);
            displaced.push(evicted.contents);
            self.metrics.evictions.inc();
        }

        self.metrics.entries.set(inner.by_seq.len() as i64);
        self.metrics.total_bytes.set(inner.total_bytes as i64);
        drop(inner);
        drop(displaced);
    }

    /// Looks up contents by sequence number.
    ///
    /// Recorded under the `by_seq` metrics label; see
    /// [`FullCheckpointContentsCacheMetrics::misses_by_seq`] for how to
    /// interpret it per node role.
    pub fn get_by_seq(&self, seq: CheckpointSequenceNumber) -> Option<Arc<FullCheckpointContents>> {
        let contents = self
            .inner
            .lock()
            .by_seq
            .get(&seq)
            .map(|entry| entry.contents.clone());
        if contents.is_some() {
            self.metrics.hits_by_seq.inc();
        } else {
            self.metrics.misses_by_seq.inc();
        }
        contents
    }

    /// Looks up contents by contents digest.
    ///
    /// Recorded under the `by_digest` metrics label; see
    /// [`FullCheckpointContentsCacheMetrics::misses_by_digest`] for how to
    /// interpret it per node role.
    pub fn get_by_digest(
        &self,
        digest: &CheckpointContentsDigest,
    ) -> Option<Arc<FullCheckpointContents>> {
        let inner = self.inner.lock();
        let contents = inner
            .seq_by_digest
            .get(digest)
            .and_then(|seq| inner.by_seq.get(seq))
            .map(|entry| entry.contents.clone());
        drop(inner);
        if contents.is_some() {
            self.metrics.hits_by_digest.inc();
        } else {
            self.metrics.misses_by_digest.inc();
        }
        contents
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(seed: u8) -> (CheckpointContentsDigest, Arc<FullCheckpointContents>) {
        let contents = FullCheckpointContents::random_for_testing();
        (
            CheckpointContentsDigest::new([seed; 32]),
            Arc::new(contents),
        )
    }

    #[test]
    fn insert_and_get_by_seq_and_digest() {
        let cache = FullCheckpointContentsCache::new(
            1000,
            FullCheckpointContentsCacheMetrics::new_for_tests(),
        );
        let (digest, contents) = entry(1);
        cache.insert(7, digest, contents.clone(), 100);

        assert!(Arc::ptr_eq(&cache.get_by_seq(7).unwrap(), &contents));
        assert!(Arc::ptr_eq(
            &cache.get_by_digest(&digest).unwrap(),
            &contents
        ));
        assert!(cache.get_by_seq(8).is_none());
        assert!(
            cache
                .get_by_digest(&CheckpointContentsDigest::new([9; 32]))
                .is_none()
        );

        assert_eq!(cache.metrics.hits_by_seq.get(), 1);
        assert_eq!(cache.metrics.misses_by_seq.get(), 1);
        assert_eq!(cache.metrics.hits_by_digest.get(), 1);
        assert_eq!(cache.metrics.misses_by_digest.get(), 1);
    }

    #[test]
    fn evicts_lowest_sequence_numbers_first() {
        let cache = FullCheckpointContentsCache::new(
            250,
            FullCheckpointContentsCacheMetrics::new_for_tests(),
        );
        for seq in 0..3 {
            let (digest, contents) = entry(seq as u8);
            cache.insert(seq, digest, contents, 100);
        }

        // 300 bytes exceed the 250-byte budget: seq 0 must be gone, 1 and 2 remain.
        assert!(cache.get_by_seq(0).is_none());
        assert!(cache.get_by_seq(1).is_some());
        assert!(cache.get_by_seq(2).is_some());
        assert_eq!(cache.metrics.evictions.get(), 1);
        assert_eq!(cache.metrics.entries.get(), 2);
        assert_eq!(cache.metrics.total_bytes.get(), 200);
    }

    #[test]
    fn digest_index_is_consistent_after_eviction() {
        let cache = FullCheckpointContentsCache::new(
            150,
            FullCheckpointContentsCacheMetrics::new_for_tests(),
        );
        let (digest_a, contents_a) = entry(1);
        let (digest_b, contents_b) = entry(2);
        cache.insert(0, digest_a, contents_a, 100);
        cache.insert(1, digest_b, contents_b, 100);

        assert!(cache.get_by_digest(&digest_a).is_none());
        assert!(cache.get_by_digest(&digest_b).is_some());
    }

    #[test]
    fn oversized_entry_is_kept_until_displaced() {
        let cache = FullCheckpointContentsCache::new(
            50,
            FullCheckpointContentsCacheMetrics::new_for_tests(),
        );
        let (digest_a, contents_a) = entry(1);
        cache.insert(0, digest_a, contents_a, 100);

        // A single entry may exceed the budget rather than being uncacheable.
        assert!(cache.get_by_seq(0).is_some());

        let (digest_b, contents_b) = entry(2);
        cache.insert(1, digest_b, contents_b, 100);
        assert!(cache.get_by_seq(0).is_none());
        assert!(cache.get_by_seq(1).is_some());
    }

    #[test]
    fn reinsert_same_sequence_replaces_entry_and_accounting() {
        let cache = FullCheckpointContentsCache::new(
            1000,
            FullCheckpointContentsCacheMetrics::new_for_tests(),
        );
        let (digest_a, contents_a) = entry(1);
        let (digest_b, contents_b) = entry(2);
        cache.insert(0, digest_a, contents_a, 100);
        cache.insert(0, digest_b, contents_b.clone(), 60);

        assert!(Arc::ptr_eq(&cache.get_by_seq(0).unwrap(), &contents_b));
        assert!(cache.get_by_digest(&digest_a).is_none());
        assert!(Arc::ptr_eq(
            &cache.get_by_digest(&digest_b).unwrap(),
            &contents_b
        ));
        assert_eq!(cache.metrics.total_bytes.get(), 60);
        assert_eq!(cache.metrics.entries.get(), 1);
    }

    #[test]
    fn eviction_keeps_digest_mapping_of_newer_entry_with_same_digest() {
        let cache = FullCheckpointContentsCache::new(
            250,
            FullCheckpointContentsCacheMetrics::new_for_tests(),
        );
        // Distinct checkpoints can share a contents digest, e.g. consecutive
        // empty checkpoints.
        let (digest, contents) = entry(1);
        cache.insert(0, digest, contents.clone(), 100);
        cache.insert(1, digest, contents, 100);

        // Push seq 0 out of the window.
        let (digest_b, contents_b) = entry(2);
        cache.insert(2, digest_b, contents_b, 100);

        assert!(cache.get_by_seq(0).is_none());
        assert!(cache.get_by_seq(1).is_some());
        // The digest lookup must keep serving the still-cached seq 1.
        assert!(cache.get_by_digest(&digest).is_some());
    }

    #[test]
    fn zero_budget_disables_the_cache() {
        let cache = FullCheckpointContentsCache::new(
            0,
            FullCheckpointContentsCacheMetrics::new_for_tests(),
        );
        let (digest_a, contents_a) = entry(1);

        assert!(!cache.should_cache(0));
        cache.insert(0, digest_a, contents_a, 100);

        assert!(cache.get_by_seq(0).is_none());
        assert!(cache.get_by_digest(&digest_a).is_none());
    }

    #[test]
    fn skips_below_window_inserts_after_eviction_under_budget() {
        let cache = FullCheckpointContentsCache::new(
            250,
            FullCheckpointContentsCacheMetrics::new_for_tests(),
        );
        // Overflow the budget so an eviction fires and total_bytes lands
        // strictly under budget — the realistic steady state, not the
        // exact-at-budget edge.
        cache.insert(10, entry(1).0, entry(1).1, 100);
        cache.insert(11, entry(2).0, entry(2).1, 100);
        cache.insert(12, entry(3).0, entry(3).1, 100);
        assert!(cache.get_by_seq(10).is_none());
        assert_eq!(cache.metrics.evictions.get(), 1);
        assert_eq!(cache.metrics.total_bytes.get(), 200); // under the 250 budget

        // Below the window: skipped even with headroom, since eviction would
        // drop it again.
        assert!(!cache.should_cache(5));
        // In or above the window: worth caching.
        assert!(cache.should_cache(11));
        assert!(cache.should_cache(13));

        // insert enforces the same guard: a below-window entry that would
        // exceed the budget is dropped without disturbing the window.
        let (digest_e, contents_e) = entry(4);
        cache.insert(5, digest_e, contents_e, 100);
        assert!(cache.get_by_seq(5).is_none());
        assert!(cache.get_by_digest(&digest_e).is_none());
        assert!(cache.get_by_seq(11).is_some());
        assert!(cache.get_by_seq(12).is_some());
        assert_eq!(cache.metrics.evictions.get(), 1);
    }
}
