// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Shared per-peer responsiveness tracking and ranking for synchronizer peer
//! selection.
//!
//! The transactions synchronizer and the commit syncer both pick peers to
//! fetch missing data from. Historically they shuffled candidates uniformly and
//! never reused the per-fetch latency they already measure. On a node with slow
//! or asymmetric inbound links this regularly draws slow-but-not-failing peers,
//! adding avoidable latency to payload/commit retrieval.
//!
//! [`PeerResponsiveness`] tracks a per-peer, per-[`DataSource`] smoothed
//! "effective latency" fed by those existing latency/outcome signals, and
//! exposes [`PeerResponsiveness::prioritize`] which reorders an
//! already-eligible candidate set to prefer responsive peers. It is a
//! preference within the candidate set, never a change of membership: the
//! caller's eligibility/safety bounds (quorum/acknowledger constraints, the
//! f+1-stake failure exclusion) are unaffected, and every fetched response is
//! still fully verified, so a peer that merely appears fast is never trusted to
//! deliver.
//!
//! Ranking is resistant to gaming: an exploration fraction of selections is a
//! plain uniform shuffle, equally-ranked peers are shuffled, and failures set a
//! timeout-scale score while successes recover through EWMA, so no peer can
//! monopolize selection and a peer that stalls or returns nothing after looking
//! fast is quickly demoted and rotated past.

use std::{sync::Arc, time::Duration};

use parking_lot::Mutex;
use rand::{Rng, seq::SliceRandom as _};
use starfish_config::{AuthorityIndex, Committee};

use crate::{dag_state::DataSource, metrics::Metrics};

impl DataSource {
    /// Number of distinct fetch sources tracked for peer responsiveness.
    pub(crate) const RESPONSIVENESS_COUNT: usize = 4;

    /// Fetch sources tracked for peer-responsiveness ranking, in index order.
    /// Only outbound, peer-selecting fetches; other sources have no peer to
    /// rank.
    pub(crate) const RESPONSIVENESS_SOURCES: [DataSource; Self::RESPONSIVENESS_COUNT] = [
        DataSource::TransactionSynchronizer,
        DataSource::CommitSyncer,
        DataSource::FastCommitSyncer,
        DataSource::HeaderSynchronizer,
    ];

    /// Dense index into the per-peer responsiveness tracker, defined only for
    /// the sources in [`Self::RESPONSIVENESS_SOURCES`]; `None` for sources with
    /// no peer to rank (own block, recovery, streaming, shard reconstruction).
    pub(crate) fn responsiveness_index(&self) -> Option<usize> {
        Some(match self {
            DataSource::TransactionSynchronizer => 0,
            DataSource::CommitSyncer => 1,
            DataSource::FastCommitSyncer => 2,
            DataSource::HeaderSynchronizer => 3,
            _ => return None,
        })
    }
}

/// Probability that a `prioritize` call ignores ranking and returns a uniform
/// shuffle, applied uniformly to every fetch kind. Guarantees every eligible
/// peer keeps a floor probability of being tried early regardless of its rank,
/// which bounds monopolization, prevents starvation of the latency tail, and
/// keeps every peer's measurement fresh.
const EXPLORE_PROBABILITY: f64 = 0.05;

/// A candidate is "fast" when its effective latency is within this multiple of
/// the fastest candidate's.
const FAST_BUCKET_RATIO: f64 = 2.0;

/// A candidate is "medium" when within this multiple of the fastest; beyond it
/// the candidate is "slow".
const MEDIUM_BUCKET_RATIO: f64 = 5.0;

/// Floor applied to every latency sample (ms). Keeps the fastest-candidate
/// reference strictly positive so bucketing can never divide-by-zero or let a
/// near-zero sample collapse all ratios.
const MIN_LATENCY_MS: f64 = 1.0;

/// Effective latency (ms) assigned to a peer with no samples yet. Sits between
/// proven-fast and proven-slow so untried peers are explored ahead of
/// known-slow peers but do not outrank peers with a proven-fast track record.
const NEUTRAL_LATENCY_MS: f64 = 250.0;

/// Minimum effective latency (ms) assigned to a failure when the caller does
/// not have a request timeout to use. Moderate (a few multiples of neutral) on
/// purpose: large enough to demote a failing peer below the responsive ones,
/// small enough that a peer recovering at the network layer climbs back within
/// a bounded number of successful fetches rather than being stuck in a penalty
/// box.
///
/// Must stay above `MEDIUM_BUCKET_RATIO * NEUTRAL_LATENCY_MS` so that a single
/// failure lands a peer in the "slow" bucket even when every other candidate is
/// untried (neutral); keep these three constants tuned together.
const FAILURE_PENALTY_MS: f64 = 1_500.0;

/// EWMA weight for a successful sample. Small, so the score is "slow to
/// trust".
const ALPHA_SUCCESS: f64 = 0.3;

// A single failure must demote a peer to the "slow" bucket even against an
// all-untried (neutral) field. Enforced at compile time so the coupled
// constants stay consistent if any is retuned.
const _: () = assert!(FAILURE_PENALTY_MS > MEDIUM_BUCKET_RATIO * NEUTRAL_LATENCY_MS);

#[derive(Clone, Default)]
struct PeerStat {
    /// Smoothed effective latency in milliseconds; `None` until the first
    /// sample.
    effective_latency_ms: Option<f64>,
    sample_origin: Option<SampleOrigin>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SampleOrigin {
    Bootstrap,
    Observed,
}

/// Per-fetch-source per-peer statistics. Each inner vector is indexed by
/// [`AuthorityIndex`] and sized to the committee, so the slot for `own_index`
/// simply stays unused.
struct Tracks {
    per_kind: [Vec<PeerStat>; DataSource::RESPONSIVENESS_COUNT],
}

/// Tracks per-peer responsiveness and ranks eligible candidates by it.
///
/// Shared per epoch behind an `Arc`; constructed once from the epoch's
/// committee. Cheap to clone (the handle is an `Arc`).
pub(crate) struct PeerResponsiveness {
    metrics: Arc<Metrics>,
    inner: Mutex<Tracks>,
}

impl PeerResponsiveness {
    pub(crate) fn new(committee: &Committee, metrics: Arc<Metrics>) -> Arc<Self> {
        let size = committee.size();
        Arc::new(Self {
            metrics,
            inner: Mutex::new(Tracks {
                per_kind: std::array::from_fn(|_| vec![PeerStat::default(); size]),
            }),
        })
    }

    /// Records a successful fetch from `peer` for `source` that took `latency`.
    ///
    /// Callers must only report latency for fetches that delivered useful data;
    /// a response that returned nothing (or a small fraction of what was
    /// requested) should be reported via [`Self::record_failure`] (or with a
    /// latency scaled up by the shortfall), so a peer cannot look fast by
    /// replying quickly with little.
    pub(crate) fn record_success(
        &self,
        source: DataSource,
        peer: AuthorityIndex,
        latency: Duration,
    ) {
        let sample = (latency.as_secs_f64() * 1_000.0).max(MIN_LATENCY_MS);
        self.update(source, peer, sample, ALPHA_SUCCESS);
    }

    /// Records a failed fetch from `peer` for `source`, demoting it to at least
    /// the default failure penalty.
    pub(crate) fn record_failure(&self, source: DataSource, peer: AuthorityIndex) {
        self.update_failure(source, peer, FAILURE_PENALTY_MS);
    }

    /// Records a failed or timed-out fetch from `peer` for `source`, demoting
    /// it to at least the operation's timeout.
    pub(crate) fn record_failure_with_timeout(
        &self,
        source: DataSource,
        peer: AuthorityIndex,
        timeout: Duration,
    ) {
        let sample = (timeout.as_secs_f64() * 1_000.0)
            .max(MIN_LATENCY_MS)
            .max(FAILURE_PENALTY_MS);
        self.update_failure(source, peer, sample);
    }

    /// Seeds an initial prior from a startup peer probe. A later observed fetch
    /// for the same source replaces this value rather than blending against it.
    pub(crate) fn record_bootstrap_success(
        &self,
        source: DataSource,
        peer: AuthorityIndex,
        latency: Duration,
    ) {
        let sample = (latency.as_secs_f64() * 1_000.0).max(MIN_LATENCY_MS);
        self.update_bootstrap(source, peer, sample);
    }

    /// Seeds an initial failure prior from a startup peer probe. Bootstrap
    /// values may be overwritten by later bootstrap or observed fetches.
    pub(crate) fn record_bootstrap_failure_with_timeout(
        &self,
        source: DataSource,
        peer: AuthorityIndex,
        timeout: Duration,
    ) {
        let sample = (timeout.as_secs_f64() * 1_000.0)
            .max(MIN_LATENCY_MS)
            .max(FAILURE_PENALTY_MS);
        self.update_bootstrap(source, peer, sample);
    }

    fn update(&self, source: DataSource, peer: AuthorityIndex, sample: f64, alpha: f64) {
        let Some(kind_index) = source.responsiveness_index() else {
            return;
        };
        let snapshot = {
            let mut tracks = self.inner.lock();
            let Some(stat) = tracks.per_kind[kind_index].get_mut(peer.value()) else {
                return;
            };
            let new = match stat.effective_latency_ms {
                None => sample,
                Some(_) if stat.sample_origin == Some(SampleOrigin::Bootstrap) => sample,
                Some(prev) => (1.0 - alpha) * prev + alpha * sample,
            };
            stat.effective_latency_ms = Some(new);
            stat.sample_origin = Some(SampleOrigin::Observed);
            Self::latency_snapshot(&tracks.per_kind[kind_index])
        };

        self.publish_bucket_counts(source, &snapshot);
    }

    fn update_failure(&self, source: DataSource, peer: AuthorityIndex, sample: f64) {
        let Some(kind_index) = source.responsiveness_index() else {
            return;
        };
        let snapshot = {
            let mut tracks = self.inner.lock();
            let Some(stat) = tracks.per_kind[kind_index].get_mut(peer.value()) else {
                return;
            };
            let new = match stat.effective_latency_ms {
                None => sample,
                Some(prev) => prev.max(sample),
            };
            stat.effective_latency_ms = Some(new);
            stat.sample_origin = Some(SampleOrigin::Observed);
            Self::latency_snapshot(&tracks.per_kind[kind_index])
        };

        self.publish_bucket_counts(source, &snapshot);
    }

    fn update_bootstrap(&self, source: DataSource, peer: AuthorityIndex, sample: f64) {
        let Some(kind_index) = source.responsiveness_index() else {
            return;
        };
        let snapshot = {
            let mut tracks = self.inner.lock();
            let Some(stat) = tracks.per_kind[kind_index].get_mut(peer.value()) else {
                return;
            };
            if stat.sample_origin == Some(SampleOrigin::Observed) {
                return;
            }
            stat.effective_latency_ms = Some(sample);
            stat.sample_origin = Some(SampleOrigin::Bootstrap);
            Self::latency_snapshot(&tracks.per_kind[kind_index])
        };

        self.publish_bucket_counts(source, &snapshot);
    }

    /// Reorders `candidates` in place to prefer peers that have been more
    /// responsive for `source`, keeping the set itself unchanged (the output is
    /// a permutation of the input: never adds or drops a peer).
    ///
    /// Ordering is a preference, not a guarantee: a fraction of calls return a
    /// uniform shuffle, equally-ranked peers are shuffled, and `rng` is
    /// consumed fresh on every call, so repeated calls do not lock onto the
    /// same peers.
    pub(crate) fn prioritize<R: Rng>(
        &self,
        source: DataSource,
        candidates: &mut [AuthorityIndex],
        rng: &mut R,
    ) {
        if candidates.len() <= 1 {
            return;
        }

        // Exploration round: ignore ranking entirely. This is the floor that
        // keeps every peer reachable early and prevents any peer from
        // monopolizing the head of the list across rounds.
        if rng.gen::<f64>() < EXPLORE_PROBABILITY {
            candidates.shuffle(rng);
            return;
        }

        let Some(kind_index) = source.responsiveness_index() else {
            return;
        };

        // Snapshot the effective latencies under the lock, then release it
        // before sorting (parking_lot::Mutex must not be held across the work).
        let scores: Vec<Option<f64>> = {
            let tracks = self.inner.lock();
            let track = &tracks.per_kind[kind_index];
            candidates
                .iter()
                .map(|peer| {
                    track
                        .get(peer.value())
                        .and_then(|stat| stat.effective_latency_ms)
                })
                .collect()
        };

        let buckets_by_position = Self::default_buckets(&scores);
        let mut buckets: std::collections::BTreeMap<AuthorityIndex, u8> =
            std::collections::BTreeMap::new();
        for (peer, bucket) in candidates.iter().zip(buckets_by_position.iter()) {
            buckets.insert(*peer, *bucket);
        }

        // Shuffle first so the order within each bucket is uniformly random
        // (randomized tie-breaking), then stable-sort by bucket so faster
        // buckets come first while preserving the shuffled intra-bucket order.
        candidates.shuffle(rng);
        candidates.sort_by_key(|peer| buckets.get(peer).copied().unwrap_or(1));
    }

    fn default_buckets(scores: &[Option<f64>]) -> Vec<u8> {
        let latencies: Vec<f64> = scores
            .iter()
            .map(|score| score.unwrap_or(NEUTRAL_LATENCY_MS))
            .collect();
        let best = latencies
            .iter()
            .copied()
            .fold(f64::INFINITY, f64::min)
            .max(MIN_LATENCY_MS);

        latencies
            .into_iter()
            .map(|latency| Self::relative_latency_bucket(latency, best))
            .collect()
    }

    fn relative_latency_bucket(latency: f64, best: f64) -> u8 {
        if latency <= FAST_BUCKET_RATIO * best {
            0
        } else if latency <= MEDIUM_BUCKET_RATIO * best {
            1
        } else {
            3
        }
    }

    fn latency_snapshot(track: &[PeerStat]) -> Vec<Option<f64>> {
        track.iter().map(|stat| stat.effective_latency_ms).collect()
    }

    /// Recomputes and publishes, for `source`, how many measured peers fall in
    /// the fast/medium/slow latency buckets relative to the fastest measured
    /// peer. Untried peers carry no sample and are not counted; all three
    /// buckets are always set so a peer leaving a bucket is reflected at once.
    fn publish_bucket_counts(&self, source: DataSource, latencies: &[Option<f64>]) {
        let (mut fast, mut medium, mut slow) = (0i64, 0i64, 0i64);
        if let Some(best) = latencies
            .iter()
            .filter_map(|latency| *latency)
            .reduce(f64::min)
            .map(|latency| latency.max(MIN_LATENCY_MS))
        {
            for latency in latencies.iter().filter_map(|latency| *latency) {
                match Self::relative_latency_bucket(latency, best) {
                    0 => fast += 1,
                    1 => medium += 1,
                    _ => slow += 1,
                }
            }
        }

        let gauge = &self
            .metrics
            .node_metrics
            .peer_responsiveness_peers_in_bucket;
        gauge
            .with_label_values(&[source.as_str(), "fast"])
            .set(fast);
        gauge
            .with_label_values(&[source.as_str(), "medium"])
            .set(medium);
        gauge
            .with_label_values(&[source.as_str(), "slow"])
            .set(slow);
    }

    #[cfg(test)]
    pub(crate) fn effective_latency_ms(
        &self,
        source: DataSource,
        peer: AuthorityIndex,
    ) -> Option<f64> {
        let kind_index = source.responsiveness_index()?;
        self.inner.lock().per_kind[kind_index]
            .get(peer.value())
            .and_then(|stat| stat.effective_latency_ms)
    }
}

#[cfg(test)]
mod tests {
    use rand::{SeedableRng, rngs::StdRng};

    use super::*;
    use crate::metrics::test_metrics;

    fn responsiveness(committee_size: usize) -> Arc<PeerResponsiveness> {
        let (committee, _) = starfish_config::local_committee_and_keys(0, vec![1; committee_size]);
        PeerResponsiveness::new(&committee, test_metrics())
    }

    fn idx(i: u8) -> AuthorityIndex {
        AuthorityIndex::new_for_test(i)
    }

    fn ms(millis: u64) -> Duration {
        Duration::from_millis(millis)
    }

    /// Counts, over `trials` seeded calls, how often each candidate ends up
    /// first. Deterministic for a fixed seed.
    fn lead_counts(
        pr: &PeerResponsiveness,
        source: DataSource,
        candidates: &[AuthorityIndex],
        trials: usize,
        seed: u64,
    ) -> std::collections::BTreeMap<AuthorityIndex, usize> {
        let mut rng = StdRng::seed_from_u64(seed);
        let mut counts = std::collections::BTreeMap::new();
        for _ in 0..trials {
            let mut c = candidates.to_vec();
            pr.prioritize(source, &mut c, &mut rng);
            *counts.entry(c[0]).or_insert(0) += 1;
        }
        counts
    }

    #[test]
    fn prioritize_empty_and_single_are_noop() {
        let pr = responsiveness(4);
        let mut rng = StdRng::seed_from_u64(1);

        let mut empty: Vec<AuthorityIndex> = vec![];
        pr.prioritize(DataSource::TransactionSynchronizer, &mut empty, &mut rng);
        assert!(empty.is_empty());

        let mut single = vec![idx(2)];
        pr.prioritize(DataSource::TransactionSynchronizer, &mut single, &mut rng);
        assert_eq!(single, vec![idx(2)]);
    }

    #[test]
    fn prioritize_preserves_membership() {
        let pr = responsiveness(7);
        // Give peers a spread of scores, including a failure and untried peers.
        pr.record_success(DataSource::CommitSyncer, idx(1), ms(10));
        pr.record_success(DataSource::CommitSyncer, idx(2), ms(900));
        pr.record_failure(DataSource::CommitSyncer, idx(3));
        // idx(4), idx(5), idx(6) remain untried.

        let candidates = vec![idx(1), idx(2), idx(3), idx(4), idx(5), idx(6)];
        for seed in 0..200u64 {
            let mut c = candidates.clone();
            let mut rng = StdRng::seed_from_u64(seed);
            pr.prioritize(DataSource::CommitSyncer, &mut c, &mut rng);
            let mut sorted = c.clone();
            sorted.sort();
            let mut expected = candidates.clone();
            expected.sort();
            assert_eq!(
                sorted, expected,
                "prioritize must be a permutation (seed {seed})"
            );
        }
    }

    #[test]
    fn prioritize_preserves_membership_with_duplicates() {
        let pr = responsiveness(4);
        pr.record_success(DataSource::TransactionSynchronizer, idx(1), ms(5));
        // A defensive check: duplicates must not be dropped.
        let candidates = vec![idx(1), idx(1), idx(2), idx(2)];
        let mut c = candidates.clone();
        let mut rng = StdRng::seed_from_u64(7);
        pr.prioritize(DataSource::TransactionSynchronizer, &mut c, &mut rng);
        let mut sorted = c.clone();
        sorted.sort();
        let mut expected = candidates;
        expected.sort();
        assert_eq!(sorted, expected);
    }

    #[test]
    fn record_success_seeds_then_smooths_ewma() {
        let pr = responsiveness(4);
        pr.record_success(DataSource::TransactionSynchronizer, idx(1), ms(100));
        assert_eq!(
            pr.effective_latency_ms(DataSource::TransactionSynchronizer, idx(1)),
            Some(100.0)
        );
        // Second sample blends with ALPHA_SUCCESS: 0.7*100 + 0.3*200 = 130.
        pr.record_success(DataSource::TransactionSynchronizer, idx(1), ms(200));
        let v = pr
            .effective_latency_ms(DataSource::TransactionSynchronizer, idx(1))
            .unwrap();
        assert!((v - 130.0).abs() < 1e-6, "got {v}");
    }

    #[test]
    fn failure_makes_a_fast_peer_slow() {
        let pr = responsiveness(4);
        pr.record_success(DataSource::CommitSyncer, idx(1), ms(10));
        let before = pr
            .effective_latency_ms(DataSource::CommitSyncer, idx(1))
            .unwrap();
        pr.record_failure(DataSource::CommitSyncer, idx(1));
        let after = pr
            .effective_latency_ms(DataSource::CommitSyncer, idx(1))
            .unwrap();
        assert!(after > before);
        assert_eq!(after, FAILURE_PENALTY_MS);
    }

    #[test]
    fn timeout_failure_sets_timeout_penalty() {
        let pr = responsiveness(4);
        pr.record_success(DataSource::HeaderSynchronizer, idx(1), ms(10));
        pr.record_failure_with_timeout(DataSource::HeaderSynchronizer, idx(1), ms(2_000));
        assert_eq!(
            pr.effective_latency_ms(DataSource::HeaderSynchronizer, idx(1)),
            Some(2_000.0)
        );
    }

    #[test]
    fn failure_never_improves_a_slow_peer() {
        let pr = responsiveness(4);
        pr.record_success(DataSource::CommitSyncer, idx(1), ms(10_000));
        pr.record_failure(DataSource::CommitSyncer, idx(1));
        assert_eq!(
            pr.effective_latency_ms(DataSource::CommitSyncer, idx(1)),
            Some(10_000.0)
        );
    }

    #[test]
    fn bootstrap_success_replaces_bootstrap_failure() {
        let pr = responsiveness(4);
        pr.record_bootstrap_failure_with_timeout(
            DataSource::TransactionSynchronizer,
            idx(1),
            ms(5_000),
        );
        pr.record_bootstrap_success(DataSource::TransactionSynchronizer, idx(1), ms(150));
        assert_eq!(
            pr.effective_latency_ms(DataSource::TransactionSynchronizer, idx(1)),
            Some(150.0)
        );
    }

    #[test]
    fn observed_success_replaces_bootstrap_prior() {
        let pr = responsiveness(4);
        pr.record_bootstrap_failure_with_timeout(
            DataSource::TransactionSynchronizer,
            idx(1),
            ms(5_000),
        );
        pr.record_success(DataSource::TransactionSynchronizer, idx(1), ms(200));
        assert_eq!(
            pr.effective_latency_ms(DataSource::TransactionSynchronizer, idx(1)),
            Some(200.0)
        );
    }

    #[test]
    fn bootstrap_does_not_override_observed_sample() {
        let pr = responsiveness(4);
        pr.record_success(DataSource::TransactionSynchronizer, idx(1), ms(200));
        pr.record_bootstrap_failure_with_timeout(
            DataSource::TransactionSynchronizer,
            idx(1),
            ms(5_000),
        );
        assert_eq!(
            pr.effective_latency_ms(DataSource::TransactionSynchronizer, idx(1)),
            Some(200.0)
        );
    }

    #[test]
    fn min_latency_floor_avoids_zero_score() {
        let pr = responsiveness(4);
        // A sub-millisecond/zero sample must be floored, never stored as 0.
        pr.record_success(
            DataSource::TransactionSynchronizer,
            idx(1),
            Duration::from_secs(0),
        );
        let v = pr
            .effective_latency_ms(DataSource::TransactionSynchronizer, idx(1))
            .unwrap();
        assert!(v >= MIN_LATENCY_MS, "got {v}");

        // And prioritize must not panic with a zero-floored best.
        let mut c = vec![idx(1), idx(2), idx(3)];
        let mut rng = StdRng::seed_from_u64(3);
        pr.prioritize(DataSource::TransactionSynchronizer, &mut c, &mut rng);
        assert_eq!(c.len(), 3);
    }

    #[test]
    fn per_kind_isolation() {
        // Each call site tracks the same peer independently, so the
        // millisecond-scale transaction track is never polluted by the
        // second-scale commit tracks (regular vs fast are also distinct).
        let pr = responsiveness(4);
        pr.record_success(DataSource::TransactionSynchronizer, idx(1), ms(5));
        pr.record_success(DataSource::CommitSyncer, idx(1), ms(5_000));
        pr.record_success(DataSource::FastCommitSyncer, idx(1), ms(9_000));
        pr.record_success(DataSource::HeaderSynchronizer, idx(1), ms(50));
        assert_eq!(
            pr.effective_latency_ms(DataSource::TransactionSynchronizer, idx(1)),
            Some(5.0)
        );
        assert_eq!(
            pr.effective_latency_ms(DataSource::CommitSyncer, idx(1)),
            Some(5_000.0)
        );
        assert_eq!(
            pr.effective_latency_ms(DataSource::FastCommitSyncer, idx(1)),
            Some(9_000.0)
        );
        assert_eq!(
            pr.effective_latency_ms(DataSource::HeaderSynchronizer, idx(1)),
            Some(50.0)
        );
    }

    #[test]
    fn fast_peer_leads_most_but_is_bounded() {
        let pr = responsiveness(5);
        // idx(1) clearly fastest; the rest are slow.
        pr.record_success(DataSource::CommitSyncer, idx(1), ms(10));
        for p in [2u8, 3, 4] {
            pr.record_success(DataSource::CommitSyncer, idx(p), ms(1_000));
        }
        let candidates = vec![idx(1), idx(2), idx(3), idx(4)];
        let trials = 10_000;
        let counts = lead_counts(&pr, DataSource::CommitSyncer, &candidates, trials, 42);
        let fast_leads = *counts.get(&idx(1)).unwrap_or(&0);
        let fast_fraction = fast_leads as f64 / trials as f64;
        // The fast peer leads far more than uniform (0.25)...
        assert!(fast_fraction > 0.5, "fast fraction {fast_fraction}");
        // ...but cannot fully monopolize: the exploration floor keeps the slow
        // peers reachable, so they still lead occasionally.
        assert!(fast_fraction < 0.99, "fast fraction {fast_fraction}");
        let slow_leads: usize = [2u8, 3, 4]
            .iter()
            .map(|p| *counts.get(&idx(*p)).unwrap_or(&0))
            .sum();
        assert!(slow_leads > 0, "slow peers must still lead sometimes");
    }

    #[test]
    fn cold_start_is_uniform() {
        let pr = responsiveness(5);
        // All untried, all neutral, all one bucket, uniform shuffle.
        let candidates = vec![idx(1), idx(2), idx(3), idx(4)];
        let counts = lead_counts(
            &pr,
            DataSource::TransactionSynchronizer,
            &candidates,
            10_000,
            7,
        );
        for p in [1u8, 2, 3, 4] {
            let fraction = *counts.get(&idx(p)).unwrap_or(&0) as f64 / 10_000.0;
            // Each of 4 peers should lead ~25% of the time.
            assert!(
                (fraction - 0.25).abs() < 0.05,
                "peer {p} fraction {fraction}"
            );
        }
    }

    #[test]
    fn transactions_prioritize_low_latency_peers_ahead_of_known_slow_tail() {
        let pr = responsiveness(50);
        for (peer, latency) in [(1, 150), (2, 250), (3, 300), (4, 450)] {
            pr.record_success(DataSource::TransactionSynchronizer, idx(peer), ms(latency));
        }
        for peer in 5..50 {
            pr.record_success(DataSource::TransactionSynchronizer, idx(peer), ms(1_200));
        }

        assert_transactions_top_four_are_low_latency_most_of_the_time(&pr);
    }

    #[test]
    fn transactions_treat_untried_peers_like_other_kinds() {
        // Bucketing is uniform across kinds: an untried transaction peer is
        // scored at the neutral prior and bucketed relative to the fastest
        // peer, exactly like every other fetch kind, rather than being forced
        // behind measured peers.
        let pr = responsiveness(6);
        // The fastest measured peer at 150ms puts the neutral prior (250ms) in
        // the fast bucket, so untried peers compete for the lead.
        pr.record_success(DataSource::TransactionSynchronizer, idx(1), ms(150));
        // idx(2)..=idx(5) are untried.
        let candidates = vec![idx(1), idx(2), idx(3), idx(4), idx(5)];
        let counts = lead_counts(
            &pr,
            DataSource::TransactionSynchronizer,
            &candidates,
            10_000,
            11,
        );
        let untried_leads: usize = [2u8, 3, 4, 5]
            .iter()
            .map(|p| *counts.get(&idx(*p)).unwrap_or(&0))
            .sum();
        // Sharing the fast bucket, untried peers lead a large share of rounds;
        // the removed transaction overlay would have capped them near the 5%
        // exploration floor.
        assert!(
            untried_leads as f64 / 10_000.0 > 0.5,
            "untried transaction peers should compete for the lead: {untried_leads}"
        );
    }

    #[test]
    fn transactions_rank_unknown_peers_ahead_of_known_slow_peers() {
        let pr = responsiveness(6);
        pr.record_success(DataSource::TransactionSynchronizer, idx(1), ms(1_200));
        pr.record_success(DataSource::TransactionSynchronizer, idx(2), ms(1_500));
        // idx(3), idx(4), idx(5) are unknown.

        let candidates = vec![idx(1), idx(2), idx(3), idx(4), idx(5)];
        let mut unknowns_before_known_slow = 0;
        for seed in 0..1_000u64 {
            let mut c = candidates.clone();
            let mut rng = StdRng::seed_from_u64(seed);
            pr.prioritize(DataSource::TransactionSynchronizer, &mut c, &mut rng);
            if c[..3]
                .iter()
                .all(|peer| [idx(3), idx(4), idx(5)].contains(peer))
            {
                unknowns_before_known_slow += 1;
            }
        }

        assert!(
            unknowns_before_known_slow > 900,
            "unknowns should fill the first three slots in most ranked transaction rounds: {unknowns_before_known_slow}"
        );
    }

    fn assert_transactions_top_four_are_low_latency_most_of_the_time(pr: &PeerResponsiveness) {
        let candidates: Vec<_> = (1..50).map(idx).collect();
        let low_latency = [idx(1), idx(2), idx(3), idx(4)];
        let mut top_four_are_low_latency = 0;

        for seed in 0..1_000u64 {
            let mut c = candidates.clone();
            let mut rng = StdRng::seed_from_u64(seed);
            pr.prioritize(DataSource::TransactionSynchronizer, &mut c, &mut rng);

            let mut sorted = c.clone();
            sorted.sort();
            assert_eq!(sorted, candidates);

            if c[..4].iter().all(|peer| low_latency.contains(peer)) {
                top_four_are_low_latency += 1;
            }
        }

        assert!(
            top_four_are_low_latency > 900,
            "low-latency peers should fill the first four slots in most transaction rounds: {top_four_are_low_latency}"
        );
    }

    #[test]
    fn transient_failure_recovers_within_bounded_successes() {
        let pr = responsiveness(4);
        pr.record_success(DataSource::CommitSyncer, idx(1), ms(20));
        pr.record_failure(DataSource::CommitSyncer, idx(1));
        let penalized = pr
            .effective_latency_ms(DataSource::CommitSyncer, idx(1))
            .unwrap();
        // A bounded number of good samples brings it back near its fast latency.
        for _ in 0..10 {
            pr.record_success(DataSource::CommitSyncer, idx(1), ms(20));
        }
        let recovered = pr
            .effective_latency_ms(DataSource::CommitSyncer, idx(1))
            .unwrap();
        assert!(recovered < penalized);
        assert!(recovered < NEUTRAL_LATENCY_MS, "recovered to {recovered}");
    }
}
