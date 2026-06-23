// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Sliding-window reputation scoring for the Starfish leader schedule.
//!
//! Maintains a running aggregate of per-commit scoring contributions (computed
//! by [`crate::leader_scoring::compute_per_commit_contribution`]) over the most
//! recent `window_size` scored commits. The aggregate updates on every commit
//! (O(1) amortized over committee size): each commit's contribution is added,
//! and once the window is full the oldest is evicted and subtracted.
//!
//! [`SlidingWindowSchedule::reputation_scores`] exposes the aggregate as
//! [`ReputationScores`] — the same type the V2 `LeaderSwapTable` is built from.
//! The sliding window is an alternative *score source* for the existing swap
//! table; it does not select leaders itself.
//!
//! # Intended usage
//!
//! This scorer is not yet wired into the leader schedule (hence the
//! `expect(dead_code)` below). Once wired, the leader schedule feeds each
//! committed subdag to [`SlidingWindowSchedule::add_commit`] in consecutive
//! index order and rebuilds the `LeaderSwapTable` from
//! [`SlidingWindowSchedule::reputation_scores`] at its usual cadence.
//!
//! On a restart or fast-sync the in-memory window starts empty and is rebuilt
//! from storage: [`SlidingWindowSchedule::replay_start`] gives the earliest
//! commit index to replay, and the committed subdags from that index up to the
//! last persisted commit are fed back through
//! [`SlidingWindowSchedule::add_commit`] to repopulate the window before any
//! scores are read.

#![cfg_attr(
    not(test),
    expect(dead_code, reason = "not yet wired into the leader schedule")
)]

use std::{collections::VecDeque, sync::Arc};

use crate::{
    commit::{CommitIndex, CommitRange, SubDagBase},
    context::Context,
    leader_scoring::{ReputationScores, compute_per_commit_contribution},
};

/// Number of pending committed subdags retained for scoring lookback.
/// `compute_per_commit_contribution` scores `c_minus_3` using `c_minus_2..=c`
/// as the lookback window.
const MAX_PENDING_COMMITS: usize = 3;

/// Sliding-window reputation scorer.
pub(crate) struct SlidingWindowSchedule {
    context: Arc<Context>,
    /// Running window length (number of scored commits aggregated).
    window_size: u32,
    /// Running aggregate, indexed by `AuthorityIndex`.
    total_scores_per_authority: Vec<u64>,
    /// Last [`MAX_PENDING_COMMITS`] committed subdags, retained for scoring
    /// lookback.
    pending_commits: VecDeque<SubDagBase>,
    /// Per-commit score contributions currently in the running window, each
    /// tagged with the index of the commit it scored. Retained so eviction can
    /// subtract them from the aggregate and so the window's commit range can be
    /// reported alongside the scores.
    scores_entries: VecDeque<(CommitIndex, Vec<u64>)>,
}

impl SlidingWindowSchedule {
    /// Creates a fresh scorer. `window_size` must be at least 1.
    pub(crate) fn new(context: Arc<Context>, window_size: u32) -> Self {
        assert!(
            window_size >= 1,
            "window_size ({window_size}) must be at least 1"
        );
        let committee_size = context.committee.size();
        Self {
            context,
            window_size,
            total_scores_per_authority: vec![0u64; committee_size],
            pending_commits: VecDeque::with_capacity(MAX_PENDING_COMMITS),
            scores_entries: VecDeque::with_capacity(window_size as usize),
        }
    }

    /// Earliest commit index a caller must replay through [`Self::add_commit`]
    /// to rebuild the window state as of `last_commit_index`.
    pub(crate) fn replay_start(last_commit_index: CommitIndex, window_size: u32) -> CommitIndex {
        // Replays a full window, not just the latest commit: the window slides by
        // adding new and subtracting evicted per-commit contributions, so recovery
        // must rebuild those per-commit entries — the aggregate sum alone cannot
        // evict. The swap table in effect at restart is recovered separately, from
        // the scores persisted in `CommitInfo`, not by this replay.
        last_commit_index
            .saturating_sub(window_size + MAX_PENDING_COMMITS as u32)
            .max(1)
    }

    /// Ingest a new committed subdag, updating the running aggregate. Commits
    /// must be supplied in consecutive index order.
    pub(crate) fn add_commit(&mut self, c: SubDagBase) {
        if let Some(last) = self.pending_commits.back() {
            assert_eq!(c.commit_ref.index, last.commit_ref.index + 1);
        }

        if self.pending_commits.len() < MAX_PENDING_COMMITS {
            self.pending_commits.push_back(c);
            return;
        }

        // Steady state: 3 commits already pending; this is the 4th. The scored
        // commit is c_minus_3 (the oldest pending).
        let c_minus_3 = &self.pending_commits[0];
        let c_minus_2 = &self.pending_commits[1];
        let c_minus_1 = &self.pending_commits[2];
        let scored_index = c_minus_3.commit_ref.index;

        let contribution =
            compute_per_commit_contribution(&self.context, c_minus_3, c_minus_2, c_minus_1, &c);

        // Evict if the window is full. saturating_sub is defensive — under normal
        // operation contributions are added and later subtracted symmetrically.
        while self.scores_entries.len() >= self.window_size as usize {
            let (_, evicted) = self
                .scores_entries
                .pop_front()
                .expect("scores_entries is non-empty when len >= window_size >= 1");
            for (i, &delta) in evicted.iter().enumerate() {
                self.total_scores_per_authority[i] =
                    self.total_scores_per_authority[i].saturating_sub(delta);
            }
        }

        // Add the new contribution.
        for (i, &delta) in contribution.iter().enumerate() {
            self.total_scores_per_authority[i] =
                self.total_scores_per_authority[i].saturating_add(delta);
        }
        self.scores_entries.push_back((scored_index, contribution));

        // Rotate pending commits: drop the now-scored c_minus_3, append c.
        self.pending_commits.pop_front();
        self.pending_commits.push_back(c);
    }

    /// Current running aggregate as [`ReputationScores`], tagged with the
    /// commit range of the scored commits in the window. Returns all-zero
    /// scores over the empty range until the first commit is scored.
    pub(crate) fn reputation_scores(&self) -> ReputationScores {
        let commit_range = match (self.scores_entries.front(), self.scores_entries.back()) {
            (Some((first, _)), Some((last, _))) => CommitRange::new(*first..=*last),
            _ => CommitRange::default(),
        };
        ReputationScores::new(commit_range, self.total_scores_per_authority.clone())
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
        let mut schedule = SlidingWindowSchedule::new(context.clone(), 10);
        let subdags = build_full_commits(context, 2);
        for s in &subdags {
            schedule.add_commit(s.clone());
        }
        assert_eq!(schedule.total_scores_per_authority, vec![0u64; 4]);
        assert_eq!(schedule.scores_entries.len(), 0);
    }

    #[tokio::test]
    async fn test_eviction_subtracts_from_aggregate() {
        // Small window so we can saturate it within a small DAG.
        let window_size: u32 = 2;
        let context = Arc::new(Context::new_for_test(4).0);
        let mut schedule = SlidingWindowSchedule::new(context.clone(), window_size);
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
    async fn test_reputation_scores_reports_window_aggregate_and_range() {
        let context = Arc::new(Context::new_for_test(4).0);
        let mut schedule = SlidingWindowSchedule::new(context.clone(), 10);

        // Before anything is scored: all-zero scores over the empty range.
        let empty = schedule.reputation_scores();
        assert_eq!(
            empty.scores_per_authority,
            vec![0u64; context.committee.size()]
        );
        assert_eq!(empty.commit_range, CommitRange::default());

        // 6 commits → commits 1..=3 scored (as c_minus_3 in iterations 4..=6).
        let subdags = build_full_commits(context.clone(), 6);
        for s in &subdags {
            schedule.add_commit(s.clone());
        }
        let scores = schedule.reputation_scores();
        assert_eq!(scores.commit_range, (1..=3).into());
        let per_commit = context.committee.total_stake();
        assert_eq!(
            scores.scores_per_authority,
            vec![per_commit * 3; context.committee.size()]
        );
    }

    #[test]
    fn test_replay_start_basic() {
        // window=10, last_commit=23 → replay_start = max(1, 23 - (10 + 3)) = 10.
        assert_eq!(SlidingWindowSchedule::replay_start(23, 10), 10);
        // Small last_commit_index → clamps to 1.
        assert_eq!(SlidingWindowSchedule::replay_start(2, 10), 1);
    }
}
