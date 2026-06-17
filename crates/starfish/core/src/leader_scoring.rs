// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, HashSet},
    fmt::Debug,
    sync::Arc,
};

use serde::{Deserialize, Serialize};
use starfish_config::AuthorityIndex;
use tracing::instrument;

use crate::{
    block_header::{BlockHeaderAPI, BlockRef, Round},
    commit::{CommitRange, SubDagBase},
    context::Context,
    stake_aggregator::{QuorumThreshold, StakeAggregator},
};

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ReputationScores {
    /// Score per authority. Vec index is the `AuthorityIndex`.
    pub(crate) scores_per_authority: Vec<u64>,
    // The range of commits these scores were calculated from.
    pub(crate) commit_range: CommitRange,
}

impl ReputationScores {
    pub(crate) fn new(commit_range: CommitRange, scores_per_authority: Vec<u64>) -> Self {
        Self {
            scores_per_authority,
            commit_range,
        }
    }

    // Returns the authorities index with score tuples.
    pub(crate) fn authorities_by_score(&self, context: Arc<Context>) -> Vec<(AuthorityIndex, u64)> {
        self.scores_per_authority
            .iter()
            .enumerate()
            .map(|(index, score)| {
                (
                    context
                        .committee
                        .to_authority_index(index)
                        .expect("Should be a valid AuthorityIndex"),
                    *score,
                )
            })
            .collect()
    }

    pub(crate) fn update_metrics(&self, context: Arc<Context>) {
        for (index, score) in self.scores_per_authority.iter().enumerate() {
            let authority_index = context
                .committee
                .to_authority_index(index)
                .expect("Should be a valid AuthorityIndex");
            let authority = context.committee.authority(authority_index);
            if !authority.hostname.is_empty() {
                context
                    .metrics
                    .node_metrics
                    .reputation_scores
                    .with_label_values(&[&authority.hostname])
                    .set(*score as i64);
            }
        }
    }

    /// Creates ReputationScores from reputation_scores_desc (as stored in
    /// commits). The reputation_scores_desc is Vec<(AuthorityIndex, u64)>
    /// sorted by score descending. This converts it to scores_per_authority
    /// indexed by authority.
    pub(crate) fn from_scores_desc(
        num_authorities: usize,
        commit_range: CommitRange,
        reputation_scores_desc: &[(AuthorityIndex, u64)],
    ) -> Self {
        let mut scores_per_authority = vec![0u64; num_authorities];
        for (authority_index, score) in reputation_scores_desc {
            scores_per_authority[*authority_index] = *score;
        }
        Self {
            scores_per_authority,
            commit_range,
        }
    }
}

/// ScoringSubdag represents the scoring votes in a collection of subdags across
/// multiple commits.
/// These subdags are "scoring" for the purposes of leader schedule change. As
/// new subdags are added, the DAG is traversed and votes for leaders are
/// recorded and scored along with stake. On a leader schedule change, finalized
/// reputation scores will be calculated based on the votes & stake collected in
/// this struct.
pub(crate) struct ScoringSubdag {
    pub(crate) context: Arc<Context>,
    pub(crate) commit_range: Option<CommitRange>,
    // Only includes committed leaders for now.
    // TODO: Include skipped leaders as well
    pub(crate) leaders: HashSet<BlockRef>,
    // A map of votes to the stake of strongly linked blocks that include that vote
    // Note: Including stake aggregator so that we can quickly check if it exceeds
    // quourum threshold and only include those scores for certain scoring strategies.
    pub(crate) votes: BTreeMap<BlockRef, StakeAggregator<QuorumThreshold>>,
}

impl ScoringSubdag {
    pub(crate) fn new(context: Arc<Context>) -> Self {
        Self {
            context,
            commit_range: None,
            leaders: HashSet::new(),
            votes: BTreeMap::new(),
        }
    }

    #[instrument(level = "trace", skip_all)]
    pub(crate) fn add_subdags(&mut self, committed_subdags: Vec<SubDagBase>) {
        let _s = self
            .context
            .metrics
            .node_metrics
            .scope_processing_time
            .with_label_values(&["ScoringSubdag::add_unscored_committed_subdags"])
            .start_timer();
        for subdag in committed_subdags {
            if let Some(commit_range) = self.commit_range.as_mut() {
                commit_range.extend_to(subdag.commit_ref.index);
            } else {
                // If the commit range is not set, then set it to the range of the first
                // committed subdag index.
                self.commit_range = Some(CommitRange::new(
                    subdag.commit_ref.index..=subdag.commit_ref.index,
                ));
            }
            // Add the committed leader to the list of leaders we will be scoring.
            tracing::trace!("Adding new committed leader {} for scoring", subdag.leader);
            self.leaders.insert(subdag.leader);
            // Check each block in subdag. Blocks are in order so we should traverse the
            // oldest blocks first
            for header in subdag.headers {
                for ancestor in header.ancestors() {
                    // Weak links may point to blocks with lower round numbers
                    // than strong links.
                    if ancestor.round != header.round().saturating_sub(1) {
                        continue;
                    }
                    // If a blocks strong linked ancestor is in leaders, then
                    // it's a vote for leader.
                    if self.leaders.contains(ancestor) {
                        // There should never be duplicate references to blocks
                        // with strong linked ancestors to leader.
                        tracing::trace!(
                            "Found a vote {} for leader {ancestor} from authority {}",
                            header.reference(),
                            header.author()
                        );
                        assert!(
                            self.votes
                                .insert(header.reference(), StakeAggregator::new())
                                .is_none(),
                            "Vote {header} already exists. Duplicate vote found for leader {ancestor}"
                        );
                    }
                    if let Some(stake) = self.votes.get_mut(ancestor) {
                        // Vote is strongly linked to a future block, so we
                        // consider this a distributed vote.
                        tracing::trace!(
                            "Found a distributed vote {ancestor} from authority {}",
                            ancestor.author
                        );
                        stake.add(header.author(), &self.context.committee);
                    }
                }
            }
        }
    }

    // Iterate through votes and calculate scores for each authority based on
    // distributed vote scoring strategy.
    pub(crate) fn calculate_distributed_vote_scores(&self) -> ReputationScores {
        let scores_per_authority = self.distributed_votes_scores();

        // TODO: Normalize scores
        ReputationScores::new(
            self.commit_range
                .clone()
                .expect("CommitRange should be set if calculate_scores is called."),
            scores_per_authority,
        )
    }

    /// This scoring strategy aims to give scores based on overall vote
    /// distribution. Instead of only giving one point for each vote that is
    /// included in 2f+1 blocks. We give a score equal to the amount of
    /// stake of all blocks that included the vote.
    fn distributed_votes_scores(&self) -> Vec<u64> {
        let _s = self
            .context
            .metrics
            .node_metrics
            .scope_processing_time
            .with_label_values(&["ScoringSubdag::score_distributed_votes"])
            .start_timer();

        let num_authorities = self.context.committee.size();
        let mut scores_per_authority = vec![0_u64; num_authorities];

        for (vote, stake_agg) in self.votes.iter() {
            let authority = vote.author;
            let stake = stake_agg.stake();
            tracing::trace!(
                "[{}] scores +{stake} reputation for {authority}!",
                self.context.own_index,
            );
            scores_per_authority[authority.value()] += stake;
        }
        scores_per_authority
    }

    pub(crate) fn scored_subdags_count(&self) -> usize {
        if let Some(commit_range) = &self.commit_range {
            commit_range.size()
        } else {
            0
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.leaders.is_empty() && self.votes.is_empty() && self.commit_range.is_none()
    }

    pub(crate) fn clear(&mut self) {
        self.leaders.clear();
        self.votes.clear();
        self.commit_range = None;
    }
}

/// Walks `[c_minus_2, c_minus_1, c]` in order, returning every commit up to and
/// **including** the first whose leader round is strictly greater than `upper`.
/// Callers pass strictly increasing leader rounds, so the scan always stops on
/// a commit above `upper` rather than running off the end.
fn scan_until_leader_round_above<'a>(
    c_minus_2: &'a SubDagBase,
    c_minus_1: &'a SubDagBase,
    c: &'a SubDagBase,
    upper: Round,
) -> Vec<&'a SubDagBase> {
    let mut out = Vec::with_capacity(3);
    for cmt in [c_minus_2, c_minus_1, c] {
        out.push(cmt);
        if cmt.leader.round > upper {
            break;
        }
    }
    out
}

/// Compute the per-commit score contribution for the new commit `c`, scoring
/// the oldest of the 3 pending commits (`c_minus_3`). Returns per-authority
/// score deltas indexed by `AuthorityIndex`.
///
/// V2's distributed-vote semantics with propagation lookahead bounded at r+2:
/// for each authority A, `contribution[A]` is the sum of stake of authorities
/// whose round-(r+2) blocks strongly link to A's round-(r+1) voting block,
/// where A's voting block strongly links to `c_minus_3`'s leader block (round
/// r).
///
/// Equivocation: if A has multiple voting blocks at r+1 within the lookback
/// window, `contribution[A] = 0`.
///
/// Assumes consecutive commits have strictly increasing leader rounds
/// (`c_minus_3 < c_minus_2 < c_minus_1 < c`), which the driving schedule
/// maintains by construction.
pub(crate) fn compute_per_commit_contribution(
    context: &Context,
    c_minus_3: &SubDagBase,
    c_minus_2: &SubDagBase,
    c_minus_1: &SubDagBase,
    c: &SubDagBase,
) -> Vec<u64> {
    let committee = &context.committee;
    let leader_ref = c_minus_3.leader;
    let r = leader_ref.round;
    let vote_round = r + 1;
    let certify_round = r + 2;

    // Voting blocks at round r+1 that strongly link to the leader block, grouped by
    // author. Multiple blocks per author indicates equivocation in the lookback
    // window.
    let voting_commits = scan_until_leader_round_above(c_minus_2, c_minus_1, c, vote_round);
    let mut voting_blocks_by_author: BTreeMap<AuthorityIndex, Vec<BlockRef>> = BTreeMap::new();
    for commit in &voting_commits {
        for header in &commit.headers {
            if header.round() != vote_round {
                continue;
            }
            if !header.ancestors().contains(&leader_ref) {
                continue;
            }
            voting_blocks_by_author
                .entry(header.author())
                .or_default()
                .push(header.reference());
        }
    }

    // Round-(r+2) certifying blocks, with their ancestor lists.
    let certifying_commits = scan_until_leader_round_above(c_minus_2, c_minus_1, c, certify_round);
    let mut certifying_blocks: Vec<(AuthorityIndex, &[BlockRef])> = Vec::new();
    for commit in &certifying_commits {
        for header in &commit.headers {
            if header.round() == certify_round {
                certifying_blocks.push((header.author(), header.ancestors()));
            }
        }
    }

    let mut scores = vec![0u64; committee.size()];
    for (author, voting_refs) in &voting_blocks_by_author {
        // Equivocation in the lookback window → zero contribution.
        if voting_refs.len() != 1 {
            continue;
        }
        let voting_ref = voting_refs[0];
        let mut certifying_stake: u64 = 0;
        for (cert_author, cert_ancestors) in &certifying_blocks {
            if cert_ancestors.contains(&voting_ref) {
                certifying_stake += committee.stake(*cert_author);
            }
        }
        scores[author.value()] = certifying_stake;
    }

    scores
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::test_dag_builder::DagBuilder;

    #[tokio::test]
    async fn test_reputation_scores_authorities_by_score() {
        let context = Arc::new(Context::new_for_test(4).0);
        let scores = ReputationScores::new((1..=300).into(), vec![4, 1, 1, 3]);
        let authorities = scores.authorities_by_score(context);
        assert_eq!(
            authorities,
            vec![
                (AuthorityIndex::new_for_test(0), 4),
                (AuthorityIndex::new_for_test(1), 1),
                (AuthorityIndex::new_for_test(2), 1),
                (AuthorityIndex::new_for_test(3), 3),
            ]
        );
    }

    #[tokio::test]
    async fn test_reputation_scores_update_metrics() {
        let context = Arc::new(Context::new_for_test(4).0);
        let scores = ReputationScores::new((1..=300).into(), vec![1, 2, 4, 3]);
        scores.update_metrics(context.clone());
        let metrics = context.metrics.node_metrics.reputation_scores.clone();
        assert_eq!(
            metrics
                .get_metric_with_label_values(&["test_host_0"])
                .unwrap()
                .get(),
            1
        );
        assert_eq!(
            metrics
                .get_metric_with_label_values(&["test_host_1"])
                .unwrap()
                .get(),
            2
        );
        assert_eq!(
            metrics
                .get_metric_with_label_values(&["test_host_2"])
                .unwrap()
                .get(),
            4
        );
        assert_eq!(
            metrics
                .get_metric_with_label_values(&["test_host_3"])
                .unwrap()
                .get(),
            3
        );
    }

    #[tokio::test]
    async fn test_scoring_subdag() {
        telemetry_subscribers::init_for_testing();
        let context = Arc::new(Context::new_for_test(4).0);
        // Populate fully connected test blocks for round 0 ~ 3, authorities 0 ~ 3.
        let mut dag_builder = DagBuilder::new(context.clone());
        dag_builder.layers(1..=3).build();
        // Build round 4 but with just the leader block
        dag_builder
            .layer(4)
            .authorities(vec![
                AuthorityIndex::new_for_test(1),
                AuthorityIndex::new_for_test(2),
                AuthorityIndex::new_for_test(3),
            ])
            .skip_block()
            .build();

        let mut scoring_subdag = ScoringSubdag::new(context);

        for (sub_dag, _commit) in dag_builder.get_sub_dag_and_commits(1..=4) {
            scoring_subdag.add_subdags(vec![sub_dag.base]);
        }

        let scores = scoring_subdag.calculate_distributed_vote_scores();
        assert_eq!(scores.scores_per_authority, vec![5, 5, 5, 5]);
        assert_eq!(scores.commit_range, (1..=4).into());
    }

    use crate::{
        block_header::BlockHeaderDigest,
        commit::{CommitDigest, CommitRef},
    };

    /// Helper: build 4 fully-connected commits using `DagBuilder` and return
    /// their `SubDagBase`s in order [commit_1, commit_2, commit_3, commit_4].
    fn build_four_commits(context: Arc<Context>) -> Vec<SubDagBase> {
        let mut dag_builder = DagBuilder::new(context);
        dag_builder.layers(1..=4).build();
        dag_builder
            .get_sub_dag_and_commits(1..=4)
            .into_iter()
            .map(|(sub_dag, _)| sub_dag.base)
            .collect()
    }

    #[tokio::test]
    async fn test_compute_per_commit_contribution_full_connectivity() {
        telemetry_subscribers::init_for_testing();
        let context = Arc::new(Context::new_for_test(4).0);
        let subdags = build_four_commits(context.clone());
        assert_eq!(subdags.len(), 4);

        let contribution = compute_per_commit_contribution(
            &context,
            &subdags[0],
            &subdags[1],
            &subdags[2],
            &subdags[3],
        );

        // In a fully-connected DAG, every authority votes for commit-1's leader and
        // every round-(r+2) block certifies every voting block. So every authority's
        // contribution equals the full committee stake.
        let expected_per_authority: u64 = context.committee.total_stake();
        assert_eq!(
            contribution,
            vec![expected_per_authority; context.committee.size()],
            "expected each authority to get full committee stake as contribution"
        );
    }

    /// Construct a `SubDagBase` with the given leader round and empty headers.
    /// Useful for tests that exercise the leader-round-based scan logic without
    /// needing a real DAG.
    fn dummy_subdag_with_leader_round(round: Round, index: u32) -> SubDagBase {
        SubDagBase {
            leader: BlockRef::new(
                round,
                AuthorityIndex::new_for_test(0),
                BlockHeaderDigest::MIN,
            ),
            headers: vec![],
            committed_header_refs: vec![],
            timestamp_ms: 0,
            commit_ref: CommitRef::new(index, CommitDigest::MIN),
            reputation_scores_desc: vec![],
        }
    }

    #[tokio::test]
    async fn test_compute_per_commit_contribution_degrades_gracefully_on_invariant_violation() {
        // Caller supplies 4 commits with non-increasing leader rounds — the
        // schedule's invariant is broken. The function must not panic; it
        // returns degraded (all-zero) scores so the node keeps running.
        let context = Arc::new(Context::new_for_test(4).0);
        let c_minus_3 = dummy_subdag_with_leader_round(5, 1);
        let c_minus_2 = dummy_subdag_with_leader_round(5, 2);
        let c_minus_1 = dummy_subdag_with_leader_round(6, 3);
        let c = dummy_subdag_with_leader_round(7, 4);
        let scores =
            compute_per_commit_contribution(&context, &c_minus_3, &c_minus_2, &c_minus_1, &c);
        assert_eq!(scores, vec![0u64; context.committee.size()]);
    }

    #[test]
    fn test_scan_returns_all_three_when_none_exceed_upper() {
        // Degenerate input: all three commits have the same leader round, so
        // none exceed `upper`. Helper returns all three rather than panicking.
        let c_minus_2 = dummy_subdag_with_leader_round(1, 2);
        let c_minus_1 = dummy_subdag_with_leader_round(1, 3);
        let c = dummy_subdag_with_leader_round(1, 4);
        let scanned = scan_until_leader_round_above(&c_minus_2, &c_minus_1, &c, /* upper */ 1);
        assert_eq!(scanned.len(), 3);
    }
}
