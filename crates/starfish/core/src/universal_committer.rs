// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::VecDeque, sync::Arc};

use parking_lot::RwLock;
use starfish_config::AuthorityIndex;

use crate::{
    base_committer::BaseCommitter,
    block_header::{GENESIS_ROUND, Round, Slot},
    commit::{CommitMetastate, DecidedLeader, Decision, LeaderStatus, WAVE_LENGTH},
    context::Context,
    dag_state::DagState,
};

#[cfg(test)]
#[path = "tests/universal_committer_tests.rs"]
mod universal_committer_tests;

#[cfg(test)]
#[path = "tests/pipelined_committer_tests.rs"]
mod pipelined_committer_tests;

/// A universal committer uses a [`BaseCommitter`] to commit a sequence of
/// leaders, one per round.
pub(crate) struct UniversalCommitter {
    /// The per-epoch configuration of this authority.
    context: Arc<Context>,
    /// In memory block store representing the dag state
    dag_state: Arc<RwLock<DagState>>,
    /// Decides the leader of every round.
    committer: BaseCommitter,
}

impl UniversalCommitter {
    /// Try to decide part of the dag. This function is idempotent and returns
    /// an ordered list of decided leaders.
    #[tracing::instrument(skip_all, fields(last_finalized = %last_finalized))]
    pub(crate) fn try_decide(&mut self, last_finalized: Slot) -> Vec<DecidedLeader> {
        // Slots up to `last_finalized` are never examined again; the next
        // slot's certifying round is `last_finalized.round + WAVE_LENGTH`.
        self.committer
            .evict_cache_below(last_finalized.round + WAVE_LENGTH);
        let highest_accepted_round = self.dag_state.read().highest_accepted_round();

        // Try to decide as many leaders as possible, starting with the highest round.
        let mut leaders: VecDeque<(LeaderStatus, Decision)> = VecDeque::new();

        let last_round = last_finalized.round + 1;

        // try to commit a leader up to the highest_accepted_round - 2. There is no
        // reason to try and iterate on higher rounds as in order to make a direct
        // decision for a leader at round R we need blocks from round R+2 to figure
        // out that enough certificates and support exist to commit a leader.
        for round in (last_round..=highest_accepted_round.saturating_sub(2)).rev() {
            let slot = self.committer.elect_leader(round);
            tracing::trace!("Trying to decide {slot}");

            let mut status = self.committer.try_direct_decide(slot);
            let mut decision = Decision::Direct;
            tracing::debug!("Outcome of direct rule: {status}");

            // If the direct result is not resolved (Commit(Pending) or
            // Undecided), run the indirect rule. For Pending, a
            // committed anchor's path can upgrade the metastate; for
            // Undecided, indirect may resolve the slot entirely.
            if !status.is_resolved() {
                let indirect = self
                    .committer
                    .try_indirect_decide(status.clone(), leaders.iter().map(|(x, _)| x));
                if indirect != status {
                    tracing::debug!("Outcome of indirect rule: {indirect}");
                    decision = match (&status, &indirect) {
                        (
                            LeaderStatus::Commit(_, CommitMetastate::Pending, _),
                            LeaderStatus::Commit(_, m, _),
                        ) if *m != CommitMetastate::Pending => Decision::Upgraded,
                        _ => Decision::Indirect,
                    };
                    status = indirect;
                }
            }
            leaders.push_front((status, decision));
        }

        // The decided sequence is the longest prefix where every leader is
        // resolved. Commit(Pending) is decided but not resolved — its metastate
        // is still pending — so sequencing stops there until a later commit pass
        // upgrades it.
        let mut decided_leaders = Vec::new();
        for (leader, decision) in leaders {
            if leader.round() == GENESIS_ROUND {
                continue;
            }
            if !leader.is_resolved() {
                break;
            }
            let decided_leader = leader
                .into_decided_leader()
                .expect("is_resolved implies a DecidedLeader");
            Self::update_metrics(&self.context, &decided_leader, decision);
            decided_leaders.push(decided_leader);
        }
        if !decided_leaders.is_empty() {
            tracing::debug!("Decided {decided_leaders:?}");
        }
        decided_leaders
    }

    /// Return list of leaders for the round. Every round has exactly one.
    pub(crate) fn get_leaders(&self, round: Round) -> Vec<AuthorityIndex> {
        vec![self.committer.elect_leader(round).authority]
    }

    /// Update metrics.
    pub(crate) fn update_metrics(
        context: &Context,
        decided_leader: &DecidedLeader,
        decision: Decision,
    ) {
        let decision_str = match decision {
            Decision::Direct => "direct",
            Decision::Upgraded => "upgraded",
            Decision::Indirect => "indirect",
        };
        let status = match decided_leader {
            DecidedLeader::Commit(_, metastate, _) => {
                format!("{decision_str}-commit-{metastate}")
            }
            DecidedLeader::Skip(..) => format!("{decision_str}-skip"),
        };
        let leader_host = &context
            .committee
            .authority(decided_leader.slot().authority)
            .hostname;
        context
            .metrics
            .node_metrics
            .decided_leaders_total
            .with_label_values(&[leader_host, &status])
            .inc();
    }
}

/// A builder for a universal committer.
pub(crate) mod universal_committer_builder {
    use super::*;
    use crate::leader_schedule::LeaderSchedule;

    pub(crate) struct UniversalCommitterBuilder {
        context: Arc<Context>,
        leader_schedule: Arc<LeaderSchedule>,
        dag_state: Arc<RwLock<DagState>>,
    }

    impl UniversalCommitterBuilder {
        pub(crate) fn new(
            context: Arc<Context>,
            leader_schedule: Arc<LeaderSchedule>,
            dag_state: Arc<RwLock<DagState>>,
        ) -> Self {
            Self {
                context,
                leader_schedule,
                dag_state,
            }
        }

        pub(crate) fn build(self) -> UniversalCommitter {
            let committer = BaseCommitter::new(
                self.context.clone(),
                self.leader_schedule,
                self.dag_state.clone(),
            );

            UniversalCommitter {
                context: self.context,
                dag_state: self.dag_state,
                committer,
            }
        }
    }
}
