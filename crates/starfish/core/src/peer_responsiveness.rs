// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Per-peer responsiveness tracking and ranking for transaction-synchronizer
//! peer selection.
//!
//! The transactions synchronizer picks peers to fetch missing transactions
//! from. Historically it shuffled candidates uniformly and never reused the
//! per-fetch latency it already measures. On a node with slow or asymmetric
//! inbound links this regularly draws slow-but-not-failing peers, adding
//! avoidable latency to payload retrieval.
//!
//! [`PeerResponsiveness`] tracks a per-peer smoothed "effective latency" fed by
//! those existing latency/outcome signals, and exposes
//! [`PeerResponsiveness::prioritize`] which reorders an already-eligible
//! candidate set to prefer responsive peers. It is a preference within the
//! candidate set, never a change of membership: the caller's eligibility/safety
//! bounds (the f+1-stake failure exclusion) are unaffected, and every fetched
//! response is still fully verified, so a peer that merely appears fast is
//! never trusted to deliver.
//!
//! Selection is a weighted random permutation: a candidate's probability of
//! being tried first is proportional to a power of its inverse effective
//! latency, so a clearly faster peer is very likely picked yet none is
//! guaranteed. An exploration fraction of selections ignores ranking and
//! shuffles uniformly, and failures set a timeout-scale score while successes
//! recover through EWMA, so no peer can monopolize selection and a peer that
//! stalls or returns nothing after looking fast is quickly demoted and rotated
//! past.

use std::{collections::HashMap, sync::Arc, time::Duration};

use parking_lot::Mutex;
use rand::{Rng, seq::SliceRandom as _};
use starfish_config::{AuthorityIndex, Committee};

use crate::{dag_state::DataSource, metrics::Metrics};

impl DataSource {
    /// Fetch sources ranked by peer responsiveness. Only the transactions
    /// synchronizer selects peers this way; other sources are not ranked.
    pub(crate) const RESPONSIVENESS_SOURCES: [DataSource; 1] =
        [DataSource::TransactionSynchronizer];
}

/// Probability that a `prioritize` call ignores ranking and returns a uniform
/// shuffle, applied uniformly to every fetch kind. Guarantees every eligible
/// peer keeps a floor probability of being tried early regardless of its rank,
/// which bounds monopolization, prevents starvation of the latency tail, and
/// keeps every peer's measurement fresh.
const EXPLORE_PROBABILITY: f64 = 0.05;

/// Exponent applied to inverse effective latency to form a peer's selection
/// weight: `weight = (1 / latency)^SELECTION_SHARPNESS`. At 1 selection is
/// literally proportional to inverse latency; higher values make a clearly
/// faster peer dominate more decisively while a small edge still spreads load.
const SELECTION_SHARPNESS: f64 = 2.0;

/// Floor applied to every latency sample (ms). Keeps effective latency strictly
/// positive so a selection weight can never divide-by-zero or let a near-zero
/// sample produce an unbounded weight.
const MIN_LATENCY_MS: f64 = 1.0;

/// Effective latency (ms) assigned to a peer with no samples yet. Sits between
/// proven-fast and proven-slow so untried peers are explored ahead of
/// known-slow peers but do not outrank peers with a proven-fast track record.
const NEUTRAL_LATENCY_MS: f64 = 250.0;

/// EWMA weight for a successful sample. Small, so the score is "slow to
/// trust".
const ALPHA_SUCCESS: f64 = 0.3;

#[derive(Clone, Default)]
struct PeerStat {
    /// Smoothed effective latency in milliseconds; `None` until the first
    /// sample.
    effective_latency_ms: Option<f64>,
}

/// Per-fetch-source per-peer statistics, keyed by the fetch source. Each vector
/// is indexed by [`AuthorityIndex`] and sized to the committee, so the slot for
/// `own_index` simply stays unused. Only the sources in
/// [`DataSource::RESPONSIVENESS_SOURCES`] are present.
struct Tracks {
    per_kind: HashMap<DataSource, Vec<PeerStat>>,
}

/// Tracks per-peer responsiveness and ranks candidates for synchronizer peer
/// selection. Shared per epoch.
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
                per_kind: DataSource::RESPONSIVENESS_SOURCES
                    .into_iter()
                    .map(|source| (source, vec![PeerStat::default(); size]))
                    .collect(),
            }),
        })
    }

    /// Records a successful fetch from `peer` for `source` that took `latency`.
    ///
    /// Callers must only report latency for fetches that delivered useful data;
    /// a response that returned nothing (or a small fraction of what was
    /// requested) should be reported via [`Self::record_failure_with_timeout`]
    /// (or with a latency scaled up by the shortfall), so a peer cannot look
    /// fast by replying quickly with little.
    pub(crate) fn record_success(
        &self,
        source: DataSource,
        peer: AuthorityIndex,
        latency: Duration,
    ) {
        let sample = (latency.as_secs_f64() * 1_000.0).max(MIN_LATENCY_MS);
        self.update(source, peer, sample, ALPHA_SUCCESS);
    }

    /// Records a failed or timed-out fetch from `peer` for `source`, demoting
    /// it to at least the operation's timeout. Floored at the neutral prior so
    /// a failure never ranks better than an untried peer.
    pub(crate) fn record_failure_with_timeout(
        &self,
        source: DataSource,
        peer: AuthorityIndex,
        timeout: Duration,
    ) {
        let sample = (timeout.as_secs_f64() * 1_000.0).max(NEUTRAL_LATENCY_MS);
        self.update_failure(source, peer, sample);
    }

    fn update(&self, source: DataSource, peer: AuthorityIndex, sample: f64, alpha: f64) {
        let snapshot = {
            let mut tracks = self.inner.lock();
            let Some(track) = tracks.per_kind.get_mut(&source) else {
                return;
            };
            let Some(stat) = track.get_mut(peer.value()) else {
                return;
            };
            let new = match stat.effective_latency_ms {
                None => sample,
                Some(prev) => (1.0 - alpha) * prev + alpha * sample,
            };
            stat.effective_latency_ms = Some(new);
            Self::latency_snapshot(track)
        };

        self.publish_expected_latencies(source, &snapshot);
    }

    fn update_failure(&self, source: DataSource, peer: AuthorityIndex, sample: f64) {
        let snapshot = {
            let mut tracks = self.inner.lock();
            let Some(track) = tracks.per_kind.get_mut(&source) else {
                return;
            };
            let Some(stat) = track.get_mut(peer.value()) else {
                return;
            };
            let new = match stat.effective_latency_ms {
                None => sample,
                Some(prev) => prev.max(sample),
            };
            stat.effective_latency_ms = Some(new);
            Self::latency_snapshot(track)
        };

        self.publish_expected_latencies(source, &snapshot);
    }

    /// Reorders `candidates` in place into a weighted random permutation that
    /// prefers peers that have been more responsive for `source`, keeping the
    /// set itself unchanged (the output is a permutation of the input: never
    /// adds or drops a peer).
    ///
    /// Each candidate's probability of landing first is proportional to its
    /// selection weight (a power of its inverse effective latency), so a
    /// clearly faster peer is very likely tried first yet none is
    /// guaranteed. A fraction of calls ignore ranking and return a uniform
    /// shuffle, and `rng` is consumed fresh on every call, so repeated
    /// calls do not lock onto the same peers.
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

        // Snapshot each candidate's selection weight under the lock, then
        // release it before drawing (parking_lot::Mutex must not be held across
        // the work). Untried peers use the neutral prior.
        let weights: Vec<f64> = {
            let tracks = self.inner.lock();
            let Some(track) = tracks.per_kind.get(&source) else {
                return;
            };
            candidates
                .iter()
                .map(|peer| {
                    let latency = track
                        .get(peer.value())
                        .and_then(|stat| stat.effective_latency_ms)
                        .unwrap_or(NEUTRAL_LATENCY_MS);
                    Self::selection_weight(latency)
                })
                .collect()
        };

        // Weighted random permutation (Efraimidis–Spirakis) in log space: the
        // key `u^(1/weight)` is monotone with `ln(u) / weight`, which avoids the
        // underflow that collapses the raw key to zero for small weights. Sorted
        // descending this yields `P(candidate first) = weight / Σ weight`,
        // extended to a full ordering, so any prefix the caller keeps is
        // weighted sampling without replacement.
        let mut keyed: Vec<(f64, AuthorityIndex)> = candidates
            .iter()
            .zip(weights)
            .map(|(peer, weight)| {
                let u = rng.gen::<f64>().max(f64::MIN_POSITIVE);
                (u.ln() / weight, *peer)
            })
            .collect();
        keyed.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        for (slot, (_, peer)) in candidates.iter_mut().zip(keyed) {
            *slot = peer;
        }
    }

    /// Selection weight for a peer with effective latency `latency_ms`: faster
    /// peers get exponentially more weight. The latency is floored so the
    /// weight stays finite.
    fn selection_weight(latency_ms: f64) -> f64 {
        (1.0 / latency_ms.max(MIN_LATENCY_MS)).powf(SELECTION_SHARPNESS)
    }

    fn latency_snapshot(track: &[PeerStat]) -> Vec<Option<f64>> {
        track.iter().map(|stat| stat.effective_latency_ms).collect()
    }

    /// Recomputes and publishes, for `source`, the expected per-fetch latency
    /// under two selection strategies over the peers measured so far: `uniform`
    /// (a random peer, the behaviour without ranking) and `weighted` (this
    /// module's responsiveness weighting, including its exploration fraction).
    /// `uniform / weighted` is the latency improvement the ranking buys; equal
    /// values mean peers are homogeneous and ranking is not helping. Untried
    /// peers carry no sample and are excluded.
    fn publish_expected_latencies(&self, source: DataSource, latencies: &[Option<f64>]) {
        let measured: Vec<f64> = latencies
            .iter()
            .filter_map(|latency| *latency)
            .map(|latency| latency.max(MIN_LATENCY_MS))
            .collect();
        if measured.is_empty() {
            return;
        }

        let uniform = measured.iter().sum::<f64>() / measured.len() as f64;

        let total_weight: f64 = measured.iter().copied().map(Self::selection_weight).sum();
        let weighted_pure = measured
            .iter()
            .map(|latency| Self::selection_weight(*latency) * latency)
            .sum::<f64>()
            / total_weight;
        // The strategy spends EXPLORE_PROBABILITY of selections on a uniform
        // draw, so its realized expected latency blends the two.
        let weighted = (1.0 - EXPLORE_PROBABILITY) * weighted_pure + EXPLORE_PROBABILITY * uniform;

        let gauge = &self
            .metrics
            .node_metrics
            .peer_responsiveness_expected_latency_ms;
        gauge
            .with_label_values(&[source.as_str(), "uniform"])
            .set(uniform);
        gauge
            .with_label_values(&[source.as_str(), "weighted"])
            .set(weighted);
    }

    #[cfg(test)]
    pub(crate) fn effective_latency_ms(
        &self,
        source: DataSource,
        peer: AuthorityIndex,
    ) -> Option<f64> {
        self.inner
            .lock()
            .per_kind
            .get(&source)?
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
        pr.record_success(DataSource::TransactionSynchronizer, idx(1), ms(10));
        pr.record_success(DataSource::TransactionSynchronizer, idx(2), ms(900));
        pr.record_failure_with_timeout(DataSource::TransactionSynchronizer, idx(3), ms(2_000));
        // idx(4), idx(5), idx(6) remain untried.

        let candidates = vec![idx(1), idx(2), idx(3), idx(4), idx(5), idx(6)];
        for seed in 0..200u64 {
            let mut c = candidates.clone();
            let mut rng = StdRng::seed_from_u64(seed);
            pr.prioritize(DataSource::TransactionSynchronizer, &mut c, &mut rng);
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
        pr.record_success(DataSource::TransactionSynchronizer, idx(1), ms(10));
        let before = pr
            .effective_latency_ms(DataSource::TransactionSynchronizer, idx(1))
            .unwrap();
        pr.record_failure_with_timeout(DataSource::TransactionSynchronizer, idx(1), ms(2_000));
        let after = pr
            .effective_latency_ms(DataSource::TransactionSynchronizer, idx(1))
            .unwrap();
        assert!(after > before);
        assert_eq!(after, 2_000.0);
    }

    #[test]
    fn timeout_failure_sets_timeout_penalty() {
        let pr = responsiveness(4);
        pr.record_success(DataSource::TransactionSynchronizer, idx(1), ms(10));
        pr.record_failure_with_timeout(DataSource::TransactionSynchronizer, idx(1), ms(2_000));
        assert_eq!(
            pr.effective_latency_ms(DataSource::TransactionSynchronizer, idx(1)),
            Some(2_000.0)
        );
    }

    #[test]
    fn failure_never_improves_a_slow_peer() {
        let pr = responsiveness(4);
        pr.record_success(DataSource::TransactionSynchronizer, idx(1), ms(10_000));
        pr.record_failure_with_timeout(DataSource::TransactionSynchronizer, idx(1), ms(2_000));
        assert_eq!(
            pr.effective_latency_ms(DataSource::TransactionSynchronizer, idx(1)),
            Some(10_000.0)
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
    fn fast_peer_leads_most_but_is_bounded() {
        let pr = responsiveness(5);
        // idx(1) clearly fastest; the rest are slow.
        pr.record_success(DataSource::TransactionSynchronizer, idx(1), ms(10));
        for p in [2u8, 3, 4] {
            pr.record_success(DataSource::TransactionSynchronizer, idx(p), ms(1_000));
        }
        let candidates = vec![idx(1), idx(2), idx(3), idx(4)];
        let trials = 10_000;
        let counts = lead_counts(
            &pr,
            DataSource::TransactionSynchronizer,
            &candidates,
            trials,
            42,
        );
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
    fn transactions_prefer_low_latency_peers_over_a_large_slow_tail() {
        let pr = responsiveness(50);
        let low_latency = [idx(1), idx(2), idx(3), idx(4)];
        for (peer, latency) in [(1, 150), (2, 250), (3, 300), (4, 450)] {
            pr.record_success(DataSource::TransactionSynchronizer, idx(peer), ms(latency));
        }
        for peer in 5..50 {
            pr.record_success(DataSource::TransactionSynchronizer, idx(peer), ms(1_200));
        }

        let candidates: Vec<_> = (1..50).map(idx).collect();
        let counts = lead_counts(
            &pr,
            DataSource::TransactionSynchronizer,
            &candidates,
            10_000,
            1,
        );

        // The four low-latency peers are only 4 of 49 candidates (uniform share
        // 8%); weighted sampling puts one of them first far more often, even
        // though a large slow tail collectively dilutes their share.
        let low_leads: usize = low_latency
            .iter()
            .map(|p| *counts.get(p).unwrap_or(&0))
            .sum();
        assert!(
            low_leads as f64 / 10_000.0 > 0.6,
            "low-latency peers should lead most rounds: {low_leads}"
        );
        // The single fastest peer leads far more than any individual slow peer.
        let fastest = *counts.get(&idx(1)).unwrap_or(&0);
        let worst_slow = (5..50)
            .map(|p| *counts.get(&idx(p)).unwrap_or(&0))
            .max()
            .unwrap_or(0);
        assert!(
            fastest > 10 * worst_slow,
            "fastest peer {fastest} should dominate any slow peer {worst_slow}"
        );
    }

    #[test]
    fn transactions_treat_untried_peers_like_other_kinds() {
        // An untried transaction peer is scored at the neutral prior and weighted
        // exactly like every other fetch kind, rather than being forced behind
        // measured peers.
        let pr = responsiveness(6);
        // The fastest measured peer at 200ms is only modestly faster than the
        // neutral prior (250ms), so untried peers keep a competitive weight.
        pr.record_success(DataSource::TransactionSynchronizer, idx(1), ms(200));
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
        // Four neutral-prior peers carry more combined weight than the single
        // slightly-faster measured peer, so they lead a large share of rounds.
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
        // idx(3), idx(4), idx(5) are unknown (neutral prior).

        let candidates = vec![idx(1), idx(2), idx(3), idx(4), idx(5)];
        let unknown = [idx(3), idx(4), idx(5)];
        let counts = lead_counts(
            &pr,
            DataSource::TransactionSynchronizer,
            &candidates,
            10_000,
            3,
        );
        let unknown_leads: usize = unknown.iter().map(|p| *counts.get(p).unwrap_or(&0)).sum();
        // Neutral-prior peers carry far more selection weight than known-slow
        // ones, so an unknown peer leads the large majority of rounds.
        assert!(
            unknown_leads as f64 / 10_000.0 > 0.85,
            "unknown peers should lead most ranked transaction rounds: {unknown_leads}"
        );
    }

    #[test]
    fn transient_failure_recovers_within_bounded_successes() {
        let pr = responsiveness(4);
        pr.record_success(DataSource::TransactionSynchronizer, idx(1), ms(20));
        pr.record_failure_with_timeout(DataSource::TransactionSynchronizer, idx(1), ms(2_000));
        let penalized = pr
            .effective_latency_ms(DataSource::TransactionSynchronizer, idx(1))
            .unwrap();
        // A bounded number of good samples brings it back near its fast latency.
        for _ in 0..10 {
            pr.record_success(DataSource::TransactionSynchronizer, idx(1), ms(20));
        }
        let recovered = pr
            .effective_latency_ms(DataSource::TransactionSynchronizer, idx(1))
            .unwrap();
        assert!(recovered < penalized);
        assert!(recovered < NEUTRAL_LATENCY_MS, "recovered to {recovered}");
    }

    #[test]
    fn expected_latency_metric_reports_weighting_improvement() {
        let pr = responsiveness(5);
        // One fast peer among slow ones: the weighted expectation should sit well
        // below the uniform average, the latency the ranking saves.
        pr.record_success(DataSource::TransactionSynchronizer, idx(1), ms(50));
        for p in [2u8, 3, 4] {
            pr.record_success(DataSource::TransactionSynchronizer, idx(p), ms(800));
        }

        let gauge = &pr
            .metrics
            .node_metrics
            .peer_responsiveness_expected_latency_ms;
        let uniform = gauge
            .with_label_values(&[DataSource::TransactionSynchronizer.as_str(), "uniform"])
            .get();
        let weighted = gauge
            .with_label_values(&[DataSource::TransactionSynchronizer.as_str(), "weighted"])
            .get();

        // Uniform is the plain average over measured peers.
        assert!(
            (uniform - (50.0 + 3.0 * 800.0) / 4.0).abs() < 1e-6,
            "uniform {uniform}"
        );
        // Weighting biases toward the fast peer, so the expectation is far lower.
        assert!(
            weighted < uniform,
            "weighted {weighted} should beat uniform {uniform}"
        );
    }
}
