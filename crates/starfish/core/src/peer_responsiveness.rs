// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Per-peer responsiveness tracking and ranking for synchronizer peer
//! selection.
//!
//! The transactions synchronizer, the commit syncer and the header
//! synchronizer pick peers to fetch from. On a node with slow or asymmetric
//! inbound links a uniform choice regularly draws slow-but-not-failing peers,
//! adding avoidable latency to payload, commit and header retrieval. Each fetch
//! source in
//! [`DataSource::RESPONSIVENESS_SOURCES`] is tracked separately, so a peer's
//! record on one kind of fetch does not decide its rank on another. Until a
//! source has a sample of its own for a peer, it places that peer by what the
//! nearest source in [`DataSource::responsiveness_fallbacks`] that has measured
//! it saw.
//!
//! [`PeerResponsiveness`] tracks a per-peer smoothed "effective latency" fed by
//! those existing latency/outcome signals, and exposes
//! [`PeerResponsiveness::prioritize`] which reorders the candidate set to
//! prefer responsive peers. It is a preference, never a change of membership:
//! the output is always a permutation of the input, and every fetched response
//! is still fully verified, so a peer that merely appears fast is never trusted
//! to deliver. A peer that fails or stalls is demoted by a timeout-scale
//! penalty and moved behind every healthy candidate rather than excluded, so
//! liveness is preserved and it recovers with its next success.
//!
//! Selection sorts healthy candidates by effective latency, permutes the
//! window of the best peers by draws with probability proportional to the
//! inverse latency — the selection shape of the checkpoint state-sync's peer
//! balancer, with a weighted draw inside the window — and keeps the remaining
//! healthy candidates in latency order. Peers whose most recent fetch failed
//! always come last, outside the randomization. So fast peers lead in
//! proportion to how fast they are, the window keeps a large slow tail from
//! diluting the weights, and a just-failed peer cannot displace a healthy
//! one. An exploration fraction of selections ignores
//! ranking (including the failure state) and shuffles uniformly, so every peer
//! keeps being sampled and a failed peer re-enters the ranked draw as soon as
//! a fetch succeeds again.

use std::{collections::HashMap, sync::Arc, time::Duration};

use parking_lot::Mutex;
use rand::{RngExt, seq::SliceRandom as _};
use starfish_config::{AuthorityIndex, Committee};

use crate::{dag_state::DataSource, metrics::Metrics};

impl DataSource {
    /// Fetch sources ranked by peer responsiveness. Sources not listed here are
    /// not ranked, and every [`PeerResponsiveness`] call for them is a no-op.
    pub(crate) const RESPONSIVENESS_SOURCES: [DataSource; 4] = [
        DataSource::TransactionSynchronizer,
        DataSource::CommitSyncer,
        DataSource::FastCommitSyncer,
        DataSource::HeaderSynchronizerRequested,
    ];

    /// Sources whose measurements order the peers this source has no sample
    /// for yet, tried in this order until one of them has measured the peer.
    ///
    /// A commit syncer reads the other flavor first (same fetches, same
    /// scale), then the header synchronizer (same endpoint, and seeded for
    /// every peer by the startup probe), then the transactions synchronizer
    /// (always populated, but its fetches are the lightest). The transactions
    /// and header synchronizers read each other, the closest scale either has.
    fn responsiveness_fallbacks(self) -> &'static [DataSource] {
        match self {
            DataSource::CommitSyncer => &[
                DataSource::FastCommitSyncer,
                DataSource::HeaderSynchronizerRequested,
                DataSource::TransactionSynchronizer,
            ],
            DataSource::FastCommitSyncer => &[
                DataSource::CommitSyncer,
                DataSource::HeaderSynchronizerRequested,
                DataSource::TransactionSynchronizer,
            ],
            DataSource::TransactionSynchronizer => &[DataSource::HeaderSynchronizerRequested],
            DataSource::HeaderSynchronizerRequested => &[DataSource::TransactionSynchronizer],
            _ => &[],
        }
    }

    /// Latency assumed for a peer that neither this source nor any of its
    /// fallbacks has measured, on the scale of this source's own fetches so
    /// that an untried peer is not placed ahead of the peers it has measured.
    fn neutral_latency_ms(self) -> f64 {
        match self {
            DataSource::CommitSyncer => COMMIT_SYNC_NEUTRAL_LATENCY_MS,
            DataSource::FastCommitSyncer => FAST_COMMIT_SYNC_NEUTRAL_LATENCY_MS,
            DataSource::HeaderSynchronizerRequested => HEADER_SYNC_NEUTRAL_LATENCY_MS,
            _ => TRANSACTIONS_SYNC_NEUTRAL_LATENCY_MS,
        }
    }
}

/// Probability that a `prioritize` call ignores ranking and returns a uniform
/// shuffle, applied uniformly to every fetch kind. Guarantees every eligible
/// peer keeps a floor probability of being tried early regardless of its rank,
/// which bounds monopolization, prevents starvation of the latency tail, and
/// keeps every peer's measurement fresh.
const EXPLORE_PROBABILITY: f64 = 0.05;

/// The weighted draw permutes only this many of the best-ranked healthy peers;
/// peers beyond the window follow in plain latency order. Bounding the draw to
/// a small window keeps the inverse-latency weights meaningful (a large slow
/// tail cannot dilute them); the sorted-then-sample-from-a-window shape
/// follows the checkpoint state-sync's `PEER_BALANCER_SELECTION_WINDOW`
/// selection, which is proven in production.
const SELECTION_WINDOW: usize = 5;

/// Floor applied to every latency sample (ms). Keeps effective latency strictly
/// positive so a selection weight can never divide-by-zero or let a near-zero
/// sample produce an unbounded weight.
const MIN_LATENCY_MS: f64 = 1.0;

/// Effective latency (ms) assigned to a peer nothing is known about yet: no
/// sample for the source being ranked, and none for any of its fallbacks
/// either. Each source has its own, on the scale its fetches measure, so that
/// an untried peer is placed among the peers that source has measured rather
/// than ahead of or behind all of them.
///
/// This one is the transactions synchronizer's, whose fetches are small and
/// frequent.
const TRANSACTIONS_SYNC_NEUTRAL_LATENCY_MS: f64 = 250.0;

/// Neutral prior for regular commit sync, which fetches the commits, their
/// headers and their transactions in requests of their own. Taken from what
/// these fetches measure on testnet and mainnet.
const COMMIT_SYNC_NEUTRAL_LATENCY_MS: f64 = 4_000.0;

/// Neutral prior for fast commit sync, which serves a range in one request
/// and so measures quicker than regular sync on the same networks.
const FAST_COMMIT_SYNC_NEUTRAL_LATENCY_MS: f64 = 2_000.0;

/// Neutral prior for the header synchronizer, rarely reached: the startup
/// probe seeds this track for every reachable peer before the first header
/// goes missing.
const HEADER_SYNC_NEUTRAL_LATENCY_MS: f64 = 500.0;

/// EWMA weight for a successful sample. Small, so the score is "slow to
/// trust".
const ALPHA_SUCCESS: f64 = 0.3;

#[derive(Clone, Default)]
struct PeerStat {
    /// Smoothed effective latency in milliseconds; `None` until the first
    /// sample.
    effective_latency_ms: Option<f64>,
    /// Whether the most recent fetch from this peer failed; cleared by the
    /// next success.
    last_fetch_failed: bool,
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
    /// it to at least the operation's timeout, and never below the neutral
    /// prior so a failure is never recorded as a fast sample. Until its next
    /// success the peer is also ordered behind every non-failed candidate
    /// (see [`Self::prioritize`]).
    pub(crate) fn record_failure_with_timeout(
        &self,
        source: DataSource,
        peer: AuthorityIndex,
        timeout: Duration,
    ) {
        let sample = (timeout.as_secs_f64() * 1_000.0).max(source.neutral_latency_ms());
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
            stat.last_fetch_failed = false;
            Self::snapshot(track)
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
            stat.last_fetch_failed = true;
            Self::snapshot(track)
        };

        self.publish_expected_latencies(source, &snapshot);
    }

    /// Reorders `candidates` in place to prefer peers that have been more
    /// responsive for `source`, keeping the set itself unchanged (the output is
    /// a permutation of the input: never adds or drops a peer).
    ///
    /// Non-failed candidates are sorted by effective latency; the
    /// [`SELECTION_WINDOW`] best are then permuted by repeated draws with
    /// probability proportional to `1 / effective_latency`, and the remaining
    /// non-failed candidates follow in latency order. A peer whose most recent
    /// fetch failed does not take part in the randomization at all: it is
    /// ordered behind every non-failed candidate (fastest of the failed first)
    /// until its next success. A fraction of calls (see
    /// [`EXPLORE_PROBABILITY`]) ignore ranking and return a uniform shuffle.
    /// `rng` is consumed fresh on every call.
    ///
    /// A candidate with no sample for `source` is placed by its latency from
    /// the nearest source in [`DataSource::responsiveness_fallbacks`] that has
    /// measured it, and by [`DataSource::neutral_latency_ms`] only when none of
    /// them has. A fallback that measures lighter fetches than `source` can put
    /// an untried peer ahead of one this source has already measured; that is
    /// the intended trade, since it is the reachable-and-quick signal available
    /// before this source has any sample of its own. Only the latency is taken
    /// from a fallback, never the failure state, so a peer that failed on
    /// another source ranks as healthy here - carrying the timeout-scale
    /// latency that failure left behind.
    pub(crate) fn prioritize<R: RngExt>(
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
        if rng.random::<f64>() < EXPLORE_PROBABILITY {
            candidates.shuffle(rng);
            return;
        }

        // Snapshot each candidate's own sample, the first fallback sample that
        // exists for it and its failure state under the lock, then release it
        // before ordering (parking_lot::Mutex must not be held across the work).
        let samples: Vec<(Option<f64>, Option<f64>, bool)> = {
            let tracks = self.inner.lock();
            let Some(track) = tracks.per_kind.get(&source) else {
                return;
            };
            let fallback_tracks: Vec<_> = source
                .responsiveness_fallbacks()
                .iter()
                .filter_map(|fallback| tracks.per_kind.get(fallback))
                .collect();
            candidates
                .iter()
                .map(|peer| {
                    let stat = track.get(peer.value());
                    let fallback_latency = fallback_tracks.iter().find_map(|track| {
                        track
                            .get(peer.value())
                            .and_then(|stat| stat.effective_latency_ms)
                    });
                    (
                        stat.and_then(|stat| stat.effective_latency_ms),
                        fallback_latency,
                        stat.is_some_and(|stat| stat.last_fetch_failed),
                    )
                })
                .collect()
        };

        // A peer with no sample for this source is placed by what the closest
        // fallback that has measured it saw, and only by the neutral prior when
        // none of them has. Every sample is floored so it stays strictly
        // positive.
        let neutral = source.neutral_latency_ms();
        let stats: Vec<(f64, bool)> = samples
            .iter()
            .map(|(own, fallback, last_fetch_failed)| {
                (
                    own.or(*fallback).unwrap_or(neutral).max(MIN_LATENCY_MS),
                    *last_fetch_failed,
                )
            })
            .collect();

        // Healthy peers are sorted by latency (ties broken by a random key so
        // an all-equal set, e.g. cold start, is ordered uniformly). The window
        // of the best peers is permuted by draws with probability proportional
        // to `1 / latency`; the rest keep their latency order. Peers whose
        // most recent fetch failed always go last, fastest first, and re-enter
        // the randomized draw with their next success.
        let mut healthy: Vec<(f64, f64, AuthorityIndex)> = Vec::new();
        let mut failed: Vec<(f64, f64, AuthorityIndex)> = Vec::new();
        for (peer, (latency, last_fetch_failed)) in candidates.iter().zip(&stats) {
            let entry = (*latency, rng.random::<f64>(), *peer);
            if *last_fetch_failed {
                failed.push(entry);
            } else {
                healthy.push(entry);
            }
        }
        let by_latency_then_key =
            |a: &(f64, f64, AuthorityIndex), b: &(f64, f64, AuthorityIndex)| {
                a.0.partial_cmp(&b.0)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            };
        healthy.sort_by(by_latency_then_key);
        failed.sort_by(by_latency_then_key);

        let mut ordered: Vec<AuthorityIndex> = Vec::with_capacity(candidates.len());
        let tail = healthy.split_off(healthy.len().min(SELECTION_WINDOW));
        while !healthy.is_empty() {
            let total: f64 = healthy.iter().map(|(latency, _, _)| 1.0 / latency).sum();
            let mut draw = rng.random::<f64>() * total;
            let mut chosen = healthy.len() - 1;
            for (i, (latency, _, _)) in healthy.iter().enumerate() {
                draw -= 1.0 / latency;
                if draw <= 0.0 {
                    chosen = i;
                    break;
                }
            }
            ordered.push(healthy.remove(chosen).2);
        }
        ordered.extend(tail.into_iter().map(|(_, _, peer)| peer));
        ordered.extend(failed.into_iter().map(|(_, _, peer)| peer));

        for (slot, peer) in candidates.iter_mut().zip(ordered) {
            *slot = peer;
        }
    }

    fn snapshot(track: &[PeerStat]) -> Vec<PeerStat> {
        track.to_vec()
    }

    /// Recomputes and publishes, for `source`, the expected per-fetch latency
    /// under two selection strategies over the peers measured so far: `uniform`
    /// (a random peer, the behaviour without ranking) and `weighted` (this
    /// module's ranking, including its exploration fraction). `uniform /
    /// weighted` is the latency improvement the ranking buys; equal values mean
    /// peers are homogeneous and ranking is not helping. Untried peers carry no
    /// sample and are excluded.
    fn publish_expected_latencies(&self, source: DataSource, stats: &[PeerStat]) {
        let measured: Vec<(f64, bool)> = stats
            .iter()
            .filter_map(|stat| {
                stat.effective_latency_ms
                    .map(|latency| (latency.max(MIN_LATENCY_MS), stat.last_fetch_failed))
            })
            .collect();
        if measured.is_empty() {
            return;
        }

        let uniform =
            measured.iter().map(|(latency, _)| latency).sum::<f64>() / measured.len() as f64;

        // The first pick is drawn from the window of the fastest healthy
        // measured peers with weight `1 / latency`; the expected latency of
        // such a draw is the harmonic mean of the window. The exploration
        // fraction falls back to a uniform draw. With every measured peer
        // failed, the draw degenerates to the failed set.
        let mut pool: Vec<f64> = measured
            .iter()
            .filter(|(_, last_fetch_failed)| !*last_fetch_failed)
            .map(|(latency, _)| *latency)
            .collect();
        if pool.is_empty() {
            pool = measured.iter().map(|(latency, _)| *latency).collect();
        }
        pool.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let window = pool.len().min(SELECTION_WINDOW);
        let harmonic_mean = window as f64
            / pool[..window]
                .iter()
                .map(|latency| 1.0 / latency)
                .sum::<f64>();
        let weighted = (1.0 - EXPLORE_PROBABILITY) * harmonic_mean + EXPLORE_PROBABILITY * uniform;

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

    /// A commit-sync track with no samples yet still orders peers, using what
    /// the transactions synchronizer has measured about the same peers.
    #[test]
    fn commit_sync_cold_start_follows_transaction_synchronizer_order() {
        let pr = responsiveness(5);
        for (peer, latency) in [(1, 400), (2, 300), (3, 200), (4, 100)] {
            pr.record_success(DataSource::TransactionSynchronizer, idx(peer), ms(latency));
        }

        let candidates = vec![idx(1), idx(2), idx(3), idx(4)];
        let counts = lead_counts(&pr, DataSource::CommitSyncer, &candidates, 10_000, 7);

        // Without the fallback all four would tie on the neutral prior and lead
        // equally often. Instead they lead in transactions-synchronizer order.
        let lead = |peer| *counts.get(&idx(peer)).unwrap_or(&0);
        assert!(
            lead(4) > lead(3) && lead(3) > lead(2) && lead(2) > lead(1),
            "lead counts should follow the fallback order, got {counts:?}"
        );
        // The fallback latencies are close together, so the weights are too:
        // the fastest peer leads more than the uniform 25% but not always.
        let fastest = lead(4) as f64 / 10_000.0;
        assert!(
            (0.30..0.55).contains(&fastest),
            "fastest peer elsewhere should lead more than uniformly, got {fastest}"
        );
    }

    /// The fallbacks are consulted in order, so a peer the other commit syncer
    /// measured is placed by that measurement even when the transactions
    /// synchronizer has a much quicker one for it.
    #[test]
    fn commit_sync_reads_the_other_commit_syncer_before_transactions() {
        for (source, other) in [
            (DataSource::CommitSyncer, DataSource::FastCommitSyncer),
            (DataSource::FastCommitSyncer, DataSource::CommitSyncer),
        ] {
            let pr = responsiveness(4);
            // Peer 1 is slow on the other commit syncer but quick on
            // transaction fetches; peer 2 is only known to the latter.
            pr.record_success(other, idx(1), ms(5_000));
            pr.record_success(DataSource::TransactionSynchronizer, idx(1), ms(10));
            pr.record_success(DataSource::TransactionSynchronizer, idx(2), ms(10));

            let counts = lead_counts(&pr, source, &[idx(1), idx(2)], 10_000, 5);

            // Reading the transactions synchronizer first would place both
            // peers at 10ms and split the lead evenly between them.
            let leads = *counts.get(&idx(2)).unwrap_or(&0) as f64 / 10_000.0;
            assert!(
                leads > 0.9,
                "{source:?} should rank by what {other:?} measured, got {counts:?}"
            );
        }
    }

    /// A transactions-synchronizer track that is still empty orders peers by
    /// what the header synchronizer saw.
    #[test]
    fn transactions_cold_start_follows_header_synchronizer_order() {
        let pr = responsiveness(5);
        for (peer, latency) in [(1, 400), (2, 300), (3, 200), (4, 100)] {
            pr.record_success(
                DataSource::HeaderSynchronizerRequested,
                idx(peer),
                ms(latency),
            );
        }

        let candidates = vec![idx(1), idx(2), idx(3), idx(4)];
        let counts = lead_counts(
            &pr,
            DataSource::TransactionSynchronizer,
            &candidates,
            10_000,
            7,
        );

        let lead = |peer| *counts.get(&idx(peer)).unwrap_or(&0);
        assert!(
            lead(4) > lead(3) && lead(3) > lead(2) && lead(2) > lead(1),
            "lead counts should follow the header-synchronizer order, got {counts:?}"
        );
    }

    /// Both commit syncers read the header synchronizer before the
    /// transactions synchronizer.
    #[test]
    fn commit_sync_reads_the_header_synchronizer_before_transactions() {
        for source in [DataSource::CommitSyncer, DataSource::FastCommitSyncer] {
            let pr = responsiveness(4);
            // Peer 1 is slow on header fetches but quick on transaction
            // fetches; peer 2 is only known to the latter.
            pr.record_success(DataSource::HeaderSynchronizerRequested, idx(1), ms(5_000));
            pr.record_success(DataSource::TransactionSynchronizer, idx(1), ms(10));
            pr.record_success(DataSource::TransactionSynchronizer, idx(2), ms(10));

            let counts = lead_counts(&pr, source, &[idx(1), idx(2)], 10_000, 5);

            // Reading the transactions synchronizer first would place both peers
            // at 10ms and split the lead evenly between them.
            let leads = *counts.get(&idx(2)).unwrap_or(&0) as f64 / 10_000.0;
            assert!(
                leads > 0.9,
                "{source:?} should rank by what the header synchronizer measured, got {counts:?}"
            );
        }
    }

    /// Fallback latencies that are all the same say nothing about which peer is
    /// quicker, so the order must stay uniform rather than following the peers'
    /// place in the candidate list.
    #[test]
    fn equal_fallback_latencies_keep_a_uniform_order() {
        let pr = responsiveness(5);
        for peer in [1, 2, 3, 4] {
            pr.record_success(DataSource::TransactionSynchronizer, idx(peer), ms(7));
        }

        let candidates = vec![idx(1), idx(2), idx(3), idx(4)];
        let counts = lead_counts(&pr, DataSource::CommitSyncer, &candidates, 10_000, 7);

        for peer in [1u8, 2, 3, 4] {
            let fraction = *counts.get(&idx(peer)).unwrap_or(&0) as f64 / 10_000.0;
            assert!(
                (fraction - 0.25).abs() < 0.05,
                "peer {peer} fraction {fraction}"
            );
        }
    }

    /// A peer with no sample for this source is placed by its fallback latency,
    /// and only by the neutral prior when the fallback has not measured it
    /// either. The fallback covers lighter fetches, so an untried peer it saw
    /// as quick can lead a peer this source measured as slow.
    #[test]
    fn untried_peers_are_placed_by_their_fallback_latency() {
        let pr = responsiveness(6);
        // Peer 1 has a commit-sync sample of its own, peers 2 and 3 only a
        // transactions-synchronizer one, and peer 4 none at all.
        pr.record_success(DataSource::CommitSyncer, idx(1), ms(100));
        pr.record_success(DataSource::TransactionSynchronizer, idx(2), ms(20));
        pr.record_success(DataSource::TransactionSynchronizer, idx(3), ms(200));

        let candidates = vec![idx(1), idx(2), idx(3), idx(4)];
        let counts = lead_counts(&pr, DataSource::CommitSyncer, &candidates, 10_000, 11);

        // Placed at 20ms, 100ms, 200ms and the commit-sync prior of 4000ms, so
        // the peer neither source has seen trails all three.
        let lead = |peer| *counts.get(&idx(peer)).unwrap_or(&0);
        assert!(
            lead(2) > lead(1) && lead(1) > lead(3) && lead(3) > lead(4),
            "peers should lead in placed-latency order, got {counts:?}"
        );
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
        // 8%); the weighted draw within the window puts one of them first far
        // more often, and a large slow tail cannot dilute the lead.
        let low_leads: usize = low_latency
            .iter()
            .map(|p| *counts.get(p).unwrap_or(&0))
            .sum();
        assert!(
            low_leads as f64 / 10_000.0 > 0.6,
            "low-latency peers should lead most rounds: {low_leads}"
        );
        // The single fastest peer leads far more than any individual slow peer
        // (weight 1/150 against 1/1200 within the shared window).
        let fastest = *counts.get(&idx(1)).unwrap_or(&0);
        let worst_slow = (5..50)
            .map(|p| *counts.get(&idx(p)).unwrap_or(&0))
            .max()
            .unwrap_or(0);
        assert!(
            fastest > 5 * worst_slow,
            "fastest peer {fastest} should dominate any slow peer {worst_slow}"
        );
    }

    #[test]
    fn measured_fast_peer_leads_more_often_than_untried_peers() {
        // The weighted draw favors a peer measured faster than the neutral
        // prior over untried peers in proportion to its weight: 1/200 against
        // 1/250 per untried peer (lead shares ~0.24 against ~0.19).
        let pr = responsiveness(6);
        pr.record_success(DataSource::TransactionSynchronizer, idx(1), ms(200));
        // idx(2)..=idx(5) are untried (neutral prior, 250ms > 200ms).
        let candidates = vec![idx(1), idx(2), idx(3), idx(4), idx(5)];
        let counts = lead_counts(
            &pr,
            DataSource::TransactionSynchronizer,
            &candidates,
            10_000,
            11,
        );
        let measured_leads = *counts.get(&idx(1)).unwrap_or(&0);
        for p in [2u8, 3, 4, 5] {
            let untried_leads = *counts.get(&idx(p)).unwrap_or(&0);
            assert!(
                measured_leads > untried_leads,
                "measured-fast peer ({measured_leads}) should lead more often \
                 than untried peer {p} ({untried_leads})"
            );
        }
    }

    #[test]
    fn peers_beyond_the_selection_window_rarely_lead() {
        // A peer ranked outside the selection window is never drawn for the
        // first position, so it leads only via exploration.
        let pr = responsiveness(13);
        for p in 1..=11u8 {
            pr.record_success(DataSource::TransactionSynchronizer, idx(p), ms(100));
        }
        pr.record_success(DataSource::TransactionSynchronizer, idx(12), ms(5_000));
        let candidates: Vec<_> = (1..=12).map(idx).collect();
        let counts = lead_counts(
            &pr,
            DataSource::TransactionSynchronizer,
            &candidates,
            10_000,
            5,
        );
        let beyond = *counts.get(&idx(12)).unwrap_or(&0);
        assert!(
            (beyond as f64) / 10_000.0 < 0.02,
            "beyond-window peer should rarely lead: {beyond}"
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
    fn last_failed_peer_goes_last_until_it_succeeds() {
        let pr = responsiveness(4);
        // idx(1) has a fast history but its most recent fetch failed;
        // idx(2) is a measured slow-but-healthy peer; idx(3) is untried.
        pr.record_success(DataSource::TransactionSynchronizer, idx(1), ms(10));
        pr.record_failure_with_timeout(DataSource::TransactionSynchronizer, idx(1), ms(2_000));
        pr.record_success(DataSource::TransactionSynchronizer, idx(2), ms(1_000));
        let candidates = vec![idx(1), idx(2), idx(3)];

        let trials = 10_000;
        let count_positions = |seed: u64| {
            let mut rng = StdRng::seed_from_u64(seed);
            let mut last = 0usize;
            let mut first = 0usize;
            for _ in 0..trials {
                let mut c = candidates.clone();
                pr.prioritize(DataSource::TransactionSynchronizer, &mut c, &mut rng);
                if c[2] == idx(1) {
                    last += 1;
                }
                if c[0] == idx(1) {
                    first += 1;
                }
            }
            (last, first)
        };

        // Outside the exploration shuffle the failed peer is always last, and
        // only exploration can put it first.
        let (last, first) = count_positions(23);
        assert!(
            last as f64 / trials as f64 > 0.93,
            "failed peer should be pinned last: {last}"
        );
        assert!(
            (first as f64) / (trials as f64) < 0.05,
            "failed peer should lead only via exploration: {first}"
        );

        // A single success clears the failure state: the peer competes in the
        // randomized tiers again and is no longer pinned last.
        pr.record_success(DataSource::TransactionSynchronizer, idx(1), ms(10));
        let (last_after, _) = count_positions(29);
        assert!(
            (last_after as f64) / (trials as f64) < 0.8,
            "recovered peer must not stay pinned last: {last_after}"
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
        assert!(
            recovered < TRANSACTIONS_SYNC_NEUTRAL_LATENCY_MS,
            "recovered to {recovered}"
        );
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
