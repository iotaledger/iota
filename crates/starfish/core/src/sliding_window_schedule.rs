// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Sliding-window leader schedule for Starfish.
//!
//! Maintains a running aggregate of per-commit scoring contributions (computed
//! by [`crate::leader_scoring::compute_per_commit_contribution`]) and exposes
//! the set of authorities currently allowed to lead. The aggregate updates on
//! every commit (O(1) work, amortized over committee size); the published
//! `allowed_leaders` set is recomputed only at `update_interval` boundaries,
//! keeping the schedule stable across short timescales while still reacting
//! within one interval of validator behavior changes.
//!
//! Single-leader regime: the per-round leader is picked as
//! `allowed_leaders[round % allowed_leaders.len()]`. Multi-leader rounds are
//! explicitly out of scope.

#![cfg_attr(
    not(test),
    expect(dead_code, reason = "not yet wired into the leader schedule")
)]

use std::{collections::VecDeque, sync::Arc};

use rand::{SeedableRng, prelude::SliceRandom, rngs::StdRng};
use starfish_config::AuthorityIndex;

use crate::{
    block_header::Round,
    commit::{CommitIndex, SubDagBase},
    context::Context,
    leader_scoring::compute_per_commit_contribution,
};

/// Number of pending committed subdags retained for scoring lookback.
/// `compute_per_commit_contribution` scores `c_minus_3` using `c_minus_2..=c`
/// as the lookback window.
const MAX_PENDING_COMMITS: usize = 3;

/// Schedule snapshot consumed by the commit rule.
///
/// `next_commit_index` and `min_next_leader_round` move on every commit;
/// `allowed_leaders` is refreshed only at `update_interval` boundaries.
#[derive(Clone, Default, Debug)]
pub(crate) struct NextCommitLeaderSchedule {
    pub(crate) next_commit_index: CommitIndex,
    pub(crate) min_next_leader_round: Round,
    pub(crate) allowed_leaders: Vec<AuthorityIndex>,
}

/// Sliding-window leader scorer.
pub(crate) struct SlidingWindowSchedule {
    context: Arc<Context>,
    /// Running window length (number of scored commits aggregated).
    window_size: u32,
    /// Commits between consecutive `allowed_leaders` recomputations.
    update_interval: u32,
    /// Running aggregate, indexed by `AuthorityIndex`.
    total_scores_per_authority: Vec<u64>,
    /// Last [`MAX_PENDING_COMMITS`] committed subdags, retained for scoring
    /// lookback.
    pending_commits: VecDeque<SubDagBase>,
    /// Per-commit score contributions currently in the running window,
    /// retained so eviction can subtract them from the aggregate.
    scores_entries: VecDeque<Vec<u64>>,
    /// Current schedule. Cached so commit-rule callers see a stable answer
    /// between rotation boundaries without repeatedly recomputing the
    /// allowed-leaders selection.
    current_schedule: NextCommitLeaderSchedule,
}

impl SlidingWindowSchedule {
    /// Creates a fresh schedule. `window_size` and `update_interval` are
    /// clamped to a minimum of 1 to avoid divide-by-zero downstream.
    pub(crate) fn new(context: Arc<Context>, window_size: u32, update_interval: u32) -> Self {
        let window_size = window_size.max(1);
        let update_interval = update_interval.max(1);
        let committee_size = context.committee.size();
        let mut schedule = Self {
            context,
            window_size,
            update_interval,
            total_scores_per_authority: vec![0u64; committee_size],
            pending_commits: VecDeque::with_capacity(MAX_PENDING_COMMITS),
            scores_entries: VecDeque::with_capacity(window_size as usize),
            current_schedule: NextCommitLeaderSchedule::default(),
        };
        schedule.current_schedule = schedule.compute_next_commit_leader_schedule();
        schedule
    }

    /// Replays a sequence of committed subdags through [`Self::add_commit`],
    /// producing the same state that live processing would yield. The caller
    /// is responsible for supplying the subdags in commit-index order, starting
    /// no later than [`Self::replay_start`] for the schedule to be fully
    /// populated.
    pub(crate) fn from_committed_subdags<I>(
        context: Arc<Context>,
        window_size: u32,
        update_interval: u32,
        subdags: I,
    ) -> Self
    where
        I: IntoIterator<Item = SubDagBase>,
    {
        let mut schedule = Self::new(context, window_size, update_interval);
        for subdag in subdags {
            schedule.add_commit(subdag);
        }
        schedule
    }

    /// Earliest commit index a caller must replay through [`Self::add_commit`]
    /// (via [`Self::from_committed_subdags`]) to reach the same state as live
    /// processing at `last_commit_index`.
    pub(crate) fn replay_start(
        last_commit_index: CommitIndex,
        window_size: u32,
        update_interval: u32,
    ) -> CommitIndex {
        let update_interval = update_interval.max(1);
        let last_schedule_update_index =
            (last_commit_index / update_interval) * update_interval + 1;
        last_schedule_update_index
            .saturating_sub(window_size + MAX_PENDING_COMMITS as u32)
            .max(1)
    }

    /// Ingest a new committed subdag, updating the running aggregate and the
    /// current schedule. Commits must be supplied in consecutive index order.
    pub(crate) fn add_commit(&mut self, c: SubDagBase) {
        if let Some(last) = self.pending_commits.back() {
            assert_eq!(c.commit_ref.index, last.commit_ref.index + 1);
        }

        if self.pending_commits.len() < MAX_PENDING_COMMITS {
            self.pending_commits.push_back(c);
            self.refresh_current_schedule();
            return;
        }

        // Steady state: 3 commits already pending; this is the 4th.
        let c_minus_3 = &self.pending_commits[0];
        let c_minus_2 = &self.pending_commits[1];
        let c_minus_1 = &self.pending_commits[2];

        let contribution =
            compute_per_commit_contribution(&self.context, c_minus_3, c_minus_2, c_minus_1, &c);

        // Evict if window full. saturating_sub is defensive — under normal
        // operation contributions are added and later subtracted symmetrically.
        while self.scores_entries.len() >= self.window_size as usize {
            let evicted = self
                .scores_entries
                .pop_front()
                .expect("scores_entries is non-empty when len >= window_size >= 1");
            for (i, &delta) in evicted.iter().enumerate() {
                self.total_scores_per_authority[i] =
                    self.total_scores_per_authority[i].saturating_sub(delta);
            }
        }

        // Add new contribution.
        for (i, &delta) in contribution.iter().enumerate() {
            self.total_scores_per_authority[i] =
                self.total_scores_per_authority[i].saturating_add(delta);
        }
        self.scores_entries.push_back(contribution);

        // Rotate pending commits: drop the now-scored c_minus_3, keep
        // c_minus_2 and c_minus_1, append c as the new newest pending.
        self.pending_commits.pop_front();
        self.pending_commits.push_back(c);

        self.refresh_current_schedule();
    }

    /// Returns the current schedule snapshot for the commit rule.
    pub(crate) fn next_commit_leader_schedule(&self) -> NextCommitLeaderSchedule {
        self.current_schedule.clone()
    }

    /// Picks the leader for `round` deterministically from the current
    /// `allowed_leaders` set. Falls back to stake-weighted base election if
    /// the set is empty (degenerate configuration).
    pub(crate) fn elect_leader(&self, round: Round) -> AuthorityIndex {
        if self.current_schedule.allowed_leaders.is_empty() {
            return self.elect_leader_stake_based(round);
        }
        let idx = (round as usize) % self.current_schedule.allowed_leaders.len();
        self.current_schedule.allowed_leaders[idx]
    }

    fn refresh_current_schedule(&mut self) {
        let next = self.next_commit_index();
        let on_boundary = next.saturating_sub(1).is_multiple_of(self.update_interval);
        if on_boundary {
            self.current_schedule = self.compute_next_commit_leader_schedule();
        } else {
            self.current_schedule.next_commit_index = next;
            self.current_schedule.min_next_leader_round = self.min_next_leader_round();
        }
    }

    fn compute_next_commit_leader_schedule(&self) -> NextCommitLeaderSchedule {
        NextCommitLeaderSchedule {
            next_commit_index: self.next_commit_index(),
            min_next_leader_round: self.min_next_leader_round(),
            allowed_leaders: self.select_allowed_leaders(),
        }
    }

    fn select_allowed_leaders(&self) -> Vec<AuthorityIndex> {
        let committee = &self.context.committee;
        let total_stake = committee.total_stake();
        let threshold_pct = self
            .context
            .protocol_config
            .consensus_bad_nodes_stake_threshold();
        let cutoff_stake = total_stake.saturating_mul(threshold_pct) / 100;

        let mut by_score: Vec<(AuthorityIndex, u64)> = self
            .total_scores_per_authority
            .iter()
            .enumerate()
            .map(|(i, &s)| {
                let authority_index = committee
                    .to_authority_index(i)
                    .expect("Score vec is indexed by valid AuthorityIndex");
                (authority_index, s)
            })
            .collect();

        let seed = self.shuffle_seed();
        let mut rng = StdRng::from_seed(seed);
        by_score.shuffle(&mut rng);
        // Stable sort by score descending; tie-breaking comes from the shuffle.
        by_score.sort_by_key(|&(_, score)| std::cmp::Reverse(score));

        let mut accumulated_bad_stake: u64 = 0;
        while let Some(&(idx, _)) = by_score.last() {
            let stake = committee.stake(idx);
            if accumulated_bad_stake.saturating_add(stake) > cutoff_stake {
                break;
            }
            accumulated_bad_stake = accumulated_bad_stake.saturating_add(stake);
            by_score.pop();
        }

        by_score.into_iter().map(|(idx, _)| idx).collect()
    }

    /// Deterministic shuffle seed derived from the most recent pending
    /// commit's digest, or an epoch-derived fallback for an empty schedule.
    fn shuffle_seed(&self) -> [u8; 32] {
        if let Some(last) = self.pending_commits.back() {
            return last.commit_ref.digest.into_inner();
        }
        let mut seed = [0u8; 32];
        seed[..8].copy_from_slice(&self.context.epoch_start_timestamp_ms.to_le_bytes());
        seed[8..16].copy_from_slice(&self.context.committee.epoch().to_le_bytes());
        seed
    }

    fn next_commit_index(&self) -> CommitIndex {
        self.pending_commits
            .back()
            .map(|c| c.commit_ref.index)
            .unwrap_or(0)
            + 1
    }

    fn min_next_leader_round(&self) -> Round {
        self.pending_commits
            .back()
            .map(|c| c.leader.round)
            .unwrap_or(0)
            + 1
    }

    fn elect_leader_stake_based(&self, round: Round) -> AuthorityIndex {
        let mut seed_bytes = [0u8; 32];
        seed_bytes[32 - 4..].copy_from_slice(&round.to_le_bytes());
        let mut rng = StdRng::from_seed(seed_bytes);
        let choices = self
            .context
            .committee
            .authorities()
            .map(|(index, authority)| (index, authority.stake as f32))
            .collect::<Vec<_>>();
        *choices
            .choose_multiple_weighted(&mut rng, self.context.committee.size(), |item| item.1)
            .expect("Weighted choice error: stake values incorrect!")
            .next()
            .map(|(index, _)| index)
            .unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_dag_builder::DagBuilder;

    fn build_full_commits(context: Arc<Context>, n: u32) -> Vec<SubDagBase> {
        let mut dag_builder = DagBuilder::new(context);
        dag_builder.layers(1..=n).build();
        dag_builder
            .get_sub_dag_and_commits(1..=n)
            .into_iter()
            .map(|(sub_dag, _)| sub_dag.base)
            .collect()
    }

    #[tokio::test]
    async fn test_warmup_no_scoring_before_three_commits() {
        let context = Arc::new(Context::new_for_test(4).0);
        let mut schedule = SlidingWindowSchedule::new(context.clone(), 10, 5);
        let subdags = build_full_commits(context, 2);
        for s in &subdags {
            schedule.add_commit(s.clone());
        }
        assert_eq!(schedule.total_scores_per_authority, vec![0u64; 4]);
        assert_eq!(schedule.scores_entries.len(), 0);
    }

    #[tokio::test]
    async fn test_full_connectivity_after_warmup() {
        let context = Arc::new(Context::new_for_test(4).0);
        let mut schedule = SlidingWindowSchedule::new(context.clone(), 10, 5);
        let subdags = build_full_commits(context.clone(), 4);
        for s in &subdags {
            schedule.add_commit(s.clone());
        }
        // 4 commits → 1 scored (c_minus_3 = commits[0], rest are lookback / new).
        assert_eq!(schedule.scores_entries.len(), 1);
        let expected = context.committee.total_stake();
        assert_eq!(
            schedule.total_scores_per_authority,
            vec![expected; context.committee.size()]
        );
    }

    #[tokio::test]
    async fn test_eviction_subtracts_from_aggregate() {
        // Small window so we can saturate it within a small DAG.
        let window_size: u32 = 2;
        let context = Arc::new(Context::new_for_test(4).0);
        let mut schedule = SlidingWindowSchedule::new(context.clone(), window_size, 1);
        // 6 commits → 3 scored (commits 1..=3 scored as c_minus_3 in iterations 4..=6).
        let subdags = build_full_commits(context.clone(), 6);
        for s in &subdags {
            schedule.add_commit(s.clone());
        }
        // Window holds at most 2 entries; 3 were added so 1 was evicted.
        assert_eq!(schedule.scores_entries.len(), window_size as usize);

        // In a fully-connected DAG each scored commit contributes
        // committee.total_stake() to every authority. The aggregate equals
        // `window_size * total_stake` because earlier contributions were
        // subtracted on eviction.
        let per_commit = context.committee.total_stake();
        let expected = per_commit * (window_size as u64);
        assert_eq!(
            schedule.total_scores_per_authority,
            vec![expected; context.committee.size()]
        );
    }

    #[tokio::test]
    async fn test_rotation_only_on_boundary() {
        let context = Arc::new(Context::new_for_test(4).0);
        let interval: u32 = 3;
        let mut schedule = SlidingWindowSchedule::new(context.clone(), 10, interval);
        let subdags = build_full_commits(context, 7);

        // Record (next_commit_index, allowed_leaders) before each add_commit.
        let mut observed: Vec<(CommitIndex, Vec<AuthorityIndex>)> = Vec::new();
        for s in &subdags {
            schedule.add_commit(s.clone());
            let snap = schedule.next_commit_leader_schedule();
            observed.push((snap.next_commit_index, snap.allowed_leaders.clone()));
        }

        // `allowed_leaders` should only ever change when crossing a rotation
        // boundary, i.e. when `(next_commit_index - 1) % interval == 0`.
        for i in 1..observed.len() {
            let (next, ref leaders) = observed[i];
            let (_, ref prev_leaders) = observed[i - 1];
            let on_boundary = next.saturating_sub(1) % interval == 0;
            if !on_boundary {
                assert_eq!(
                    leaders, prev_leaders,
                    "allowed_leaders changed off-boundary at next_commit_index={next}"
                );
            }
        }
    }

    #[tokio::test]
    async fn test_deterministic_across_instances() {
        let context = Arc::new(Context::new_for_test(4).0);
        let subdags = build_full_commits(context.clone(), 6);

        let mut a = SlidingWindowSchedule::new(context.clone(), 10, 3);
        let mut b = SlidingWindowSchedule::new(context, 10, 3);
        for s in &subdags {
            a.add_commit(s.clone());
            b.add_commit(s.clone());
        }

        assert_eq!(a.total_scores_per_authority, b.total_scores_per_authority);
        let sched_a = a.next_commit_leader_schedule();
        let sched_b = b.next_commit_leader_schedule();
        assert_eq!(sched_a.next_commit_index, sched_b.next_commit_index);
        assert_eq!(sched_a.min_next_leader_round, sched_b.min_next_leader_round);
        assert_eq!(sched_a.allowed_leaders, sched_b.allowed_leaders);
    }

    #[tokio::test]
    async fn test_from_committed_subdags_matches_live() {
        let context = Arc::new(Context::new_for_test(4).0);
        let subdags = build_full_commits(context.clone(), 6);

        let mut live = SlidingWindowSchedule::new(context.clone(), 10, 3);
        for s in &subdags {
            live.add_commit(s.clone());
        }

        let replayed =
            SlidingWindowSchedule::from_committed_subdags(context, 10, 3, subdags.iter().cloned());

        assert_eq!(
            live.total_scores_per_authority,
            replayed.total_scores_per_authority
        );
        let s_live = live.next_commit_leader_schedule();
        let s_replay = replayed.next_commit_leader_schedule();
        assert_eq!(s_live.next_commit_index, s_replay.next_commit_index);
        assert_eq!(s_live.min_next_leader_round, s_replay.min_next_leader_round);
        assert_eq!(s_live.allowed_leaders, s_replay.allowed_leaders);
    }

    #[tokio::test]
    async fn test_elect_leader_round_robin_over_allowed() {
        let context = Arc::new(Context::new_for_test(4).0);
        let mut schedule = SlidingWindowSchedule::new(context.clone(), 10, 1);
        let subdags = build_full_commits(context, 5);
        for s in &subdags {
            schedule.add_commit(s.clone());
        }
        let allowed = schedule.next_commit_leader_schedule().allowed_leaders;
        assert!(!allowed.is_empty(), "allowed_leaders should be non-empty");
        // Round-robin: for several rounds, elect_leader matches the expected index.
        for r in 0..(2 * allowed.len() as Round) {
            let expected = allowed[(r as usize) % allowed.len()];
            assert_eq!(schedule.elect_leader(r), expected);
        }
    }

    #[tokio::test]
    async fn test_elect_leader_falls_back_when_allowed_empty() {
        // Force allowed_leaders empty to exercise the stake-based fallback.
        // Under normal configuration the threshold pop never empties the set,
        // so this requires direct manipulation.
        let context = Arc::new(Context::new_for_test(4).0);
        let mut schedule = SlidingWindowSchedule::new(context.clone(), 10, 5);
        schedule.current_schedule.allowed_leaders.clear();
        let leader = schedule.elect_leader(0);
        // Fallback must return a valid authority index.
        assert!(leader.value() < context.committee.size());
    }

    #[test]
    fn test_replay_start_basic() {
        // window=10, interval=5, last_commit=23 → last_schedule_update_index = (23/5)*5
        // + 1 = 21 replay_start = max(1, 21 - (10 + 3)) = 8
        assert_eq!(SlidingWindowSchedule::replay_start(23, 10, 5), 8);
        // Small last_commit_index → clamps to 1.
        assert_eq!(SlidingWindowSchedule::replay_start(2, 10, 5), 1);
        // update_interval=0 is clamped to 1.
        assert_eq!(SlidingWindowSchedule::replay_start(10, 5, 0), 3);
    }

    #[test]
    fn test_new_clamps_zero_params() {
        let context = Arc::new(Context::new_for_test(4).0);
        let schedule = SlidingWindowSchedule::new(context, 0, 0);
        assert_eq!(schedule.window_size, 1);
        assert_eq!(schedule.update_interval, 1);
    }
}
