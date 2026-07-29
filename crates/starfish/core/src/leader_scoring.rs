// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fmt::Debug,
    sync::Arc,
};

use serde::{Deserialize, Serialize};
use starfish_config::AuthorityIndex;
use tracing::instrument;

use crate::{
    block_header::{BlockHeaderAPI, BlockRef},
    commit::{CommitRange, SubDagBase},
    context::Context,
    error::{ConsensusError, ConsensusResult},
    stake_aggregator::{QuorumThreshold, StakeAggregator},
};

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ReputationScores {
    /// Score per authority. Vec index is the `AuthorityIndex`.
    pub(crate) scores_per_authority: Vec<u64>,
    /// The range of commits these scores were calculated from. Exception: on
    /// the sliding-window path the *persisted* range is the committed
    /// interval ending at the rotation boundary,
    /// whereas the scores are aggregated over a deeper window ending
    /// `MAX_PENDING_COMMITS` commits earlier.
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
    ///
    /// Returns `InvalidAuthorityIndex` if an authority index is out of range
    /// for the committee; the scores originate from fetched commits.
    pub(crate) fn from_scores_desc(
        num_authorities: usize,
        commit_range: CommitRange,
        reputation_scores_desc: &[(AuthorityIndex, u64)],
    ) -> ConsensusResult<Self> {
        let mut scores_per_authority = vec![0u64; num_authorities];
        for (authority_index, score) in reputation_scores_desc {
            let slot = scores_per_authority
                .get_mut(authority_index.value())
                .ok_or(ConsensusError::InvalidAuthorityIndex {
                    index: *authority_index,
                    max: num_authorities,
                })?;
            *slot = *score;
        }
        Ok(Self {
            scores_per_authority,
            commit_range,
        })
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

        // The vote traversal below feeds only the V2 boundary score computation;
        // the sliding-window path scores from its running window aggregate
        // instead. There only `commit_range` is needed (it drives rotation
        // timing), so skip the per-commit traversal.
        let sliding_window_enabled = self
            .context
            .protocol_config
            .consensus_enable_sliding_window_leader_schedule();

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

            if sliding_window_enabled {
                continue;
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

/// Reputation multiplier applied to a strong vote relative to an ordinary vote.
/// A strong vote (only produced under `consensus_starfish_speed`) is a stronger
/// signal that the voter promptly saw and backed the leader — it attests
/// holding the leader's transaction data, not just its header — so it scores
/// double. This scorer only runs under
/// `consensus_enable_sliding_window_leader_schedule`, so the weighting ships
/// as part of the redesigned schedule and V2 scoring is unaffected.
const STRONG_VOTE_MULTIPLIER: u64 = 2;

/// Compute the per-commit score contribution for the new commit `c`, scoring
/// the oldest of the 3 pending commits (`c_minus_3`). Returns per-authority
/// score deltas indexed by `AuthorityIndex`.
///
/// V2's distributed-vote semantics with propagation lookahead bounded at r+2:
/// for each authority A, `contribution[A]` is the sum of stake of authorities
/// whose round-(r+2) blocks strongly link to A's round-(r+1) voting block,
/// where A's voting block strongly links to `c_minus_3`'s leader block (round
/// r). The round-(r+1) and round-(r+2) blocks are collected from all three
/// commits following `c_minus_3` (`c_minus_2`, `c_minus_1`, `c`).
///
/// Under `consensus_starfish_speed`, a voting block that is a *strong* vote for
/// the leader has its contribution multiplied by [`STRONG_VOTE_MULTIPLIER`].
///
/// Equivocation: if A has multiple voting blocks at r+1 within that window,
/// `contribution[A] = 0`.
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

    // Strong votes score double. Gate on the flag so a stray strong_vote on a
    // non-speed block can never change scoring (strong votes only exist under
    // starfish speed).
    let double_strong_votes = context.protocol_config.consensus_starfish_speed();

    // Voting blocks at round r+1 that strongly link to the leader block, grouped by
    // author. Multiple blocks per author indicates equivocation in the lookback
    // window. `strong_by_ref` records whether each voting block is a strong vote
    // for the leader.
    let mut voting_blocks_by_author: BTreeMap<AuthorityIndex, Vec<BlockRef>> = BTreeMap::new();
    let mut strong_by_ref: HashMap<BlockRef, bool> = HashMap::new();
    for commit in [c_minus_2, c_minus_1, c] {
        for header in &commit.headers {
            if header.round() != vote_round {
                continue;
            }
            if !header.ancestors().contains(&leader_ref) {
                continue;
            }
            let vote_ref = header.reference();
            voting_blocks_by_author
                .entry(header.author())
                .or_default()
                .push(vote_ref);
            strong_by_ref.insert(vote_ref, header.is_strong_vote_for(leader_ref.author));
        }
    }

    // Voting-block ref → author, for authors with exactly one voting block.
    // Equivocation in the lookback window → excluded, zero contribution.
    // `strong_voter[A]` is true when A's single voting block is a strong vote.
    let mut voting_author_by_ref: HashMap<BlockRef, AuthorityIndex> = HashMap::new();
    let mut strong_voter = vec![false; committee.size()];
    for (author, voting_refs) in &voting_blocks_by_author {
        if let [voting_ref] = voting_refs.as_slice() {
            voting_author_by_ref.insert(*voting_ref, *author);
            strong_voter[author.value()] = strong_by_ref.get(voting_ref).copied().unwrap_or(false);
        }
    }

    // One pass over the round-(r+2) blocks' ancestors. The aggregator dedups
    // by certifier so an equivocating certifier's stake counts once per author.
    let mut certifying_stake: Vec<StakeAggregator<QuorumThreshold>> = (0..committee.size())
        .map(|_| StakeAggregator::new())
        .collect();
    for commit in [c_minus_2, c_minus_1, c] {
        for header in &commit.headers {
            if header.round() != certify_round {
                continue;
            }
            for ancestor in header.ancestors() {
                if let Some(author) = voting_author_by_ref.get(ancestor) {
                    certifying_stake[author.value()].add(header.author(), committee);
                }
            }
        }
    }

    certifying_stake
        .iter()
        .enumerate()
        .map(|(author, agg)| {
            if double_strong_votes && strong_voter[author] {
                agg.stake() * STRONG_VOTE_MULTIPLIER
            } else {
                agg.stake()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::{
        authority_set::AuthoritySet,
        block_header::{
            BlockHeaderDigest, Round, StrongVote, TestBlockHeader, VerifiedBlockHeader,
        },
        commit::{CommitDigest, CommitRef},
        test_dag_builder::DagBuilder,
    };

    #[test]
    fn test_from_scores_desc_converts_to_per_authority_scores() {
        let scores = vec![
            (AuthorityIndex::new_for_test(1), 10),
            (AuthorityIndex::new_for_test(3), 5),
        ];
        let reputation_scores =
            ReputationScores::from_scores_desc(4, (1..=10).into(), &scores).unwrap();
        assert_eq!(reputation_scores.scores_per_authority, vec![0, 10, 0, 5]);
    }

    #[test]
    fn test_from_scores_desc_rejects_out_of_range_authority() {
        let scores = vec![(AuthorityIndex::new_for_test(4), 10)];
        let result = ReputationScores::from_scores_desc(4, (1..=10).into(), &scores);
        assert!(matches!(
            result,
            Err(ConsensusError::InvalidAuthorityIndex { index, max })
                if index == AuthorityIndex::new_for_test(4) && max == 4
        ));
    }

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

    #[tokio::test]
    async fn test_compute_per_commit_contribution_dedups_equivocating_certifier() {
        // An equivocating certifier — two round-(r+2) blocks from one author,
        // both linking the same voting block — contributes its stake once, not
        // once per block.
        let context = Arc::new(Context::new_for_test(4).0);
        let committee = &context.committee;
        let r: Round = 10;

        // Minimal subdag: the scorer reads only the leader round and the
        // headers' ancestor links.
        let subdag = |leader_round: Round, headers: Vec<VerifiedBlockHeader>| SubDagBase {
            leader: BlockRef::new(
                leader_round,
                AuthorityIndex::new_for_test(0),
                BlockHeaderDigest::MIN,
            ),
            headers,
            committed_header_refs: vec![],
            timestamp_ms: 0,
            commit_ref: CommitRef::new(0, CommitDigest::MIN),
            reputation_scores_desc: vec![],
        };

        let c_minus_3 = subdag(r, vec![]);

        // Authority 0 casts a single vote for the leader at round r+1.
        let voting_block = VerifiedBlockHeader::new_for_test(
            TestBlockHeader::new(r + 1, 0)
                .set_ancestors(vec![c_minus_3.leader])
                .build(),
        );
        let voting_ref = voting_block.reference();

        // Authority 1 equivocates at round r+2 with two distinct blocks, each
        // certifying the voting block.
        let cert_a = VerifiedBlockHeader::new_for_test(
            TestBlockHeader::new(r + 2, 1)
                .set_ancestors(vec![voting_ref])
                .set_timestamp_ms(1)
                .build(),
        );
        let cert_b = VerifiedBlockHeader::new_for_test(
            TestBlockHeader::new(r + 2, 1)
                .set_ancestors(vec![voting_ref])
                .set_timestamp_ms(2)
                .build(),
        );

        let c_minus_2 = subdag(r + 1, vec![voting_block]);
        let c_minus_1 = subdag(r + 2, vec![cert_a, cert_b]);
        let c = subdag(r + 3, vec![]);

        let scores =
            compute_per_commit_contribution(&context, &c_minus_3, &c_minus_2, &c_minus_1, &c);

        let mut expected = vec![0u64; committee.size()];
        expected[0] = committee.stake(AuthorityIndex::new_for_test(1));
        assert_eq!(
            scores, expected,
            "equivocating certifier's stake must be counted once, not per block"
        );
    }

    #[tokio::test]
    async fn test_compute_per_commit_contribution_attributes_partial_certification() {
        // Each voting author is certified by a different subset of round-(r+2)
        // blocks, so every authority expects a distinct score and any
        // misattribution is visible.
        let context = Arc::new(Context::new_for_test(4).0);
        let committee = &context.committee;
        let r: Round = 10;

        let subdag = |leader_round: Round, headers: Vec<VerifiedBlockHeader>| SubDagBase {
            leader: BlockRef::new(
                leader_round,
                AuthorityIndex::new_for_test(0),
                BlockHeaderDigest::MIN,
            ),
            headers,
            committed_header_refs: vec![],
            timestamp_ms: 0,
            commit_ref: CommitRef::new(0, CommitDigest::MIN),
            reputation_scores_desc: vec![],
        };

        let c_minus_3 = subdag(r, vec![]);

        // Authorities 0, 1, 2 vote for the leader at round r+1; authority 3's
        // round-(r+1) block does not link the leader, so it is not a vote.
        let vote = |author: u8| {
            VerifiedBlockHeader::new_for_test(
                TestBlockHeader::new(r + 1, author)
                    .set_ancestors(vec![c_minus_3.leader])
                    .build(),
            )
        };
        let vote_0 = vote(0);
        let vote_1 = vote(1);
        let vote_2 = vote(2);
        let non_vote_3 = VerifiedBlockHeader::new_for_test(TestBlockHeader::new(r + 1, 3).build());

        // Certifiers at round r+2: authority 0 links all round-(r+1) blocks
        // (including the non-vote, which must credit no one), authority 1
        // links votes {0, 1}, authority 2 links vote {0} only.
        let cert = |author: u8, ancestors: Vec<BlockRef>| {
            VerifiedBlockHeader::new_for_test(
                TestBlockHeader::new(r + 2, author)
                    .set_ancestors(ancestors)
                    .build(),
            )
        };
        let cert_0 = cert(
            0,
            vec![
                vote_0.reference(),
                vote_1.reference(),
                vote_2.reference(),
                non_vote_3.reference(),
            ],
        );
        let cert_1 = cert(1, vec![vote_0.reference(), vote_1.reference()]);
        let cert_2 = cert(2, vec![vote_0.reference()]);

        // Spread the blocks across the three commits following `c_minus_3`.
        let c_minus_2 = subdag(r + 1, vec![vote_0, vote_1, vote_2, non_vote_3]);
        let c_minus_1 = subdag(r + 2, vec![cert_0, cert_1]);
        let c = subdag(r + 3, vec![cert_2]);

        let scores =
            compute_per_commit_contribution(&context, &c_minus_3, &c_minus_2, &c_minus_1, &c);

        let stake = |index: u8| committee.stake(AuthorityIndex::new_for_test(index));
        let expected = vec![
            stake(0) + stake(1) + stake(2),
            stake(0) + stake(1),
            stake(0),
            0,
        ];
        assert_eq!(
            scores, expected,
            "each voting author must be credited exactly its own certifiers' stake"
        );
    }

    #[tokio::test]
    async fn test_compute_per_commit_contribution_doubles_strong_vote() {
        telemetry_subscribers::init_for_testing();
        // Strong votes score double under starfish speed.
        let mut ctx = Context::new_for_test(4).0;
        ctx.protocol_config
            .set_consensus_starfish_speed_for_testing(true);
        let context = Arc::new(ctx);
        let committee = &context.committee;
        let r: Round = 10;
        let leader_authority = AuthorityIndex::new_for_test(0);

        let subdag = |leader_round: Round, headers: Vec<VerifiedBlockHeader>| SubDagBase {
            leader: BlockRef::new(leader_round, leader_authority, BlockHeaderDigest::MIN),
            headers,
            committed_header_refs: vec![],
            timestamp_ms: 0,
            commit_ref: CommitRef::new(0, CommitDigest::MIN),
            reputation_scores_desc: vec![],
        };

        let c_minus_3 = subdag(r, vec![]);

        // Authority 1 casts an ordinary vote; authority 2 casts a strong vote
        // for the same leader. Both link the leader at round r+1.
        let ordinary_vote = VerifiedBlockHeader::new_for_test(
            TestBlockHeader::new(r + 1, 1)
                .set_ancestors(vec![c_minus_3.leader])
                .build(),
        );
        let strong_vote = VerifiedBlockHeader::new_for_test(
            TestBlockHeader::new(r + 1, 2)
                .set_ancestors(vec![c_minus_3.leader])
                .set_strong_vote(Some(StrongVote {
                    leader_authority,
                    missing: AuthoritySet::new(),
                }))
                .build(),
        );
        let ordinary_ref = ordinary_vote.reference();
        let strong_ref = strong_vote.reference();

        // One certifier (authority 3) certifies both voting blocks at r+2, so
        // both voters collect the same certifying stake — the only difference
        // is that the strong vote is doubled.
        let certifier = VerifiedBlockHeader::new_for_test(
            TestBlockHeader::new(r + 2, 3)
                .set_ancestors(vec![ordinary_ref, strong_ref])
                .build(),
        );

        let c_minus_2 = subdag(r + 1, vec![ordinary_vote, strong_vote]);
        let c_minus_1 = subdag(r + 2, vec![certifier]);
        let c = subdag(r + 3, vec![]);

        let scores =
            compute_per_commit_contribution(&context, &c_minus_3, &c_minus_2, &c_minus_1, &c);

        let stake_3 = committee.stake(AuthorityIndex::new_for_test(3));
        assert_eq!(
            scores[1], stake_3,
            "ordinary vote earns the certifying stake"
        );
        assert_eq!(scores[2], 2 * stake_3, "strong vote earns double");
    }

    #[tokio::test]
    async fn test_add_subdags_skips_vote_scoring_when_sliding_window_enabled() {
        let mut context = Context::new_for_test(4).0;
        context
            .protocol_config
            .set_consensus_enable_sliding_window_leader_schedule_for_testing(true);
        let context = Arc::new(context);

        // A fully-connected DAG that would produce non-zero votes on the V2 path.
        let mut dag_builder = DagBuilder::new(context.clone());
        dag_builder.layers(1..=4).build();

        let mut scoring_subdag = ScoringSubdag::new(context);
        for (sub_dag, _commit) in dag_builder.get_sub_dag_and_commits(1..=4) {
            scoring_subdag.add_subdags(vec![sub_dag.base]);
        }

        // The commit range is still maintained — it is what drives rotation
        // timing (`scored_subdags_count` / `is_empty`) on the sliding-window path.
        assert_eq!(scoring_subdag.scored_subdags_count(), 4);
        assert_eq!(scoring_subdag.commit_range, Some(CommitRange::new(1..=4)));
        assert!(!scoring_subdag.is_empty());

        // But the per-commit vote traversal was skipped: no leaders or votes
        // accumulated, so the V2 boundary score computation is never fed.
        assert!(scoring_subdag.leaders.is_empty());
        assert!(scoring_subdag.votes.is_empty());
    }
}
