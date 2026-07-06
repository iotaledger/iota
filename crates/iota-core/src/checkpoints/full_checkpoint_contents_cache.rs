// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use iota_config::node::DEFAULT_FULL_CHECKPOINT_CONTENTS_CACHE_SIZE_MB;
use iota_types::{
    digests::CheckpointContentsDigest,
    messages_checkpoint::{CheckpointSequenceNumber, FullCheckpointContents},
};
use parking_lot::Mutex;
use prometheus_filtered::{
    IntCounter, IntCounterVec, IntGauge, Registry, register_int_counter_vec_with_registry,
    register_int_counter_with_registry, register_int_gauge_with_registry,
};

/// Value of the `lookup` metrics label for sequence-number lookups: the
/// checkpoint executor's bulk load and `ReadStore` reads by sequence number.
const LOOKUP_BY_SEQ: &str = "by_seq";
/// Value of the `lookup` metrics label for contents-digest lookups: serving
/// checkpoint-contents requests from state-sync peers.
const LOOKUP_BY_DIGEST: &str = "by_digest";

pub struct FullContentsCacheMetrics {
    /// Lookups served from the cache, partitioned by the `lookup` label
    /// ([`LOOKUP_BY_SEQ`] / [`LOOKUP_BY_DIGEST`]).
    pub hits: IntCounterVec,
    /// Lookups that missed the cache, with the same `lookup` label split as
    /// `hits`.
    ///
    /// On validators, `by_seq` misses accrue once per self-produced
    /// checkpoint by design: the executor checks the cache before inserting
    /// the entry it then produces. `by_digest` is the health signal for
    /// whether peers are served from memory or fall back to reconstruction.
    pub misses: IntCounterVec,
    pub evictions: IntCounter,
    pub entries: IntGauge,
    pub total_bytes: IntGauge,
}

impl FullContentsCacheMetrics {
    pub fn new(registry: &Registry) -> Arc<Self> {
        Arc::new(Self {
            hits: register_int_counter_vec_with_registry!(
                "full_checkpoint_contents_cache_hits",
                "Number of full-checkpoint-contents cache lookups served from the cache, \
                partitioned by lookup kind",
                &["lookup"],
                registry
            )
            .unwrap(),
            misses: register_int_counter_vec_with_registry!(
                "full_checkpoint_contents_cache_misses",
                "Number of full-checkpoint-contents cache lookups that missed the cache, \
                partitioned by lookup kind",
                &["lookup"],
                registry
            )
            .unwrap(),
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
/// tip-following peers request. A single entry larger than the whole budget is
/// still cached until the next insert displaces it. A budget of zero disables
/// the cache entirely.
pub struct FullCheckpointContentsCache {
    max_bytes: usize,
    metrics: Arc<FullContentsCacheMetrics>,
    inner: Mutex<Inner>,
}

impl Default for FullCheckpointContentsCache {
    /// A cache with the default byte budget and unregistered metrics, for
    /// contexts without a node config or metrics registry (tools, tests).
    fn default() -> Self {
        Self::new(
            DEFAULT_FULL_CHECKPOINT_CONTENTS_CACHE_SIZE_MB * 1024 * 1024,
            FullContentsCacheMetrics::new_for_tests(),
        )
    }
}

impl FullCheckpointContentsCache {
    pub fn new(max_bytes: usize, metrics: Arc<FullContentsCacheMetrics>) -> Self {
        Self {
            max_bytes,
            metrics,
            inner: Mutex::new(Inner::default()),
        }
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

        if let Some(old) = inner.by_seq.insert(
            seq,
            CacheEntry {
                digest,
                contents,
                bytes,
            },
        ) {
            inner.total_bytes -= old.bytes;
            inner.seq_by_digest.remove(&old.digest);
        }
        inner.total_bytes += bytes;
        inner.seq_by_digest.insert(digest, seq);

        while inner.total_bytes > self.max_bytes && inner.by_seq.len() > 1 {
            let (_, evicted) = inner.by_seq.pop_first().expect("cache is not empty");
            inner.total_bytes -= evicted.bytes;
            inner.seq_by_digest.remove(&evicted.digest);
            self.metrics.evictions.inc();
        }

        self.metrics.entries.set(inner.by_seq.len() as i64);
        self.metrics.total_bytes.set(inner.total_bytes as i64);
    }

    /// Looks up contents by sequence number.
    ///
    /// Recorded under the `by_seq` metrics label; see
    /// [`FullContentsCacheMetrics::misses`] for how to interpret it per node
    /// role.
    pub fn get_by_seq(&self, seq: CheckpointSequenceNumber) -> Option<Arc<FullCheckpointContents>> {
        let contents = self
            .inner
            .lock()
            .by_seq
            .get(&seq)
            .map(|entry| entry.contents.clone());
        self.record_lookup(contents.is_some(), LOOKUP_BY_SEQ);
        contents
    }

    /// Looks up contents by contents digest.
    ///
    /// Recorded under the `by_digest` metrics label; see
    /// [`FullContentsCacheMetrics::misses`] for how to interpret it per node
    /// role.
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
        self.record_lookup(contents.is_some(), LOOKUP_BY_DIGEST);
        contents
    }

    fn record_lookup(&self, hit: bool, lookup: &str) {
        if hit {
            self.metrics.hits.with_label_values(&[lookup]).inc();
        } else {
            self.metrics.misses.with_label_values(&[lookup]).inc();
        }
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
        let cache =
            FullCheckpointContentsCache::new(1000, FullContentsCacheMetrics::new_for_tests());
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

        for lookup in [LOOKUP_BY_SEQ, LOOKUP_BY_DIGEST] {
            assert_eq!(cache.metrics.hits.with_label_values(&[lookup]).get(), 1);
            assert_eq!(cache.metrics.misses.with_label_values(&[lookup]).get(), 1);
        }
    }

    #[test]
    fn evicts_lowest_sequence_numbers_first() {
        let cache =
            FullCheckpointContentsCache::new(250, FullContentsCacheMetrics::new_for_tests());
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
        let cache =
            FullCheckpointContentsCache::new(150, FullContentsCacheMetrics::new_for_tests());
        let (digest_a, contents_a) = entry(1);
        let (digest_b, contents_b) = entry(2);
        cache.insert(0, digest_a, contents_a, 100);
        cache.insert(1, digest_b, contents_b, 100);

        assert!(cache.get_by_digest(&digest_a).is_none());
        assert!(cache.get_by_digest(&digest_b).is_some());
    }

    #[test]
    fn oversized_entry_is_kept_until_displaced() {
        let cache = FullCheckpointContentsCache::new(50, FullContentsCacheMetrics::new_for_tests());
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
        let cache =
            FullCheckpointContentsCache::new(1000, FullContentsCacheMetrics::new_for_tests());
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
    fn zero_budget_disables_the_cache() {
        let cache = FullCheckpointContentsCache::new(0, FullContentsCacheMetrics::new_for_tests());
        let (digest_a, contents_a) = entry(1);
        cache.insert(0, digest_a, contents_a, 100);

        assert!(cache.get_by_seq(0).is_none());
        assert!(cache.get_by_digest(&digest_a).is_none());
    }
}
