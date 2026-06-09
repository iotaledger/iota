// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeMap, fmt::Display};

use iota_types::{digests::ConsensusCommitDigest, messages_consensus::ConsensusTransaction};
use itertools::Itertools as _;

use crate::consensus_types::AuthorityIndex;
/// A list of tuples of:
/// (block origin authority index, all transactions contained in the block).
/// For each transaction, returns deserialized transaction and its serialized
/// size.
type ConsensusOutputTransactions = Vec<(AuthorityIndex, Vec<(ConsensusTransaction, usize)>)>;

/// Per-authority misbehavior counts observed by consensus in a single commit.
/// A bridge type between Starfish's internal observation state and IOTA's
/// wire format: the trait impl on `starfish_core::CommittedSubDag` is
/// responsible for extracting the counts; `observations_from_consensus_output`
/// projects this struct onto whichever `MisbehaviorObservationsVN` is active
/// for the current protocol version.
///
/// Per-field empty `Vec`s mean "not wired yet" and are zero-filled by the
/// mapper. Today this struct is structurally identical to
/// `MisbehaviorObservationsV1`; the separation exists so consensus's
/// observation set and the wire schema can evolve on independent cadences.
#[derive(Debug, Default)]
pub struct ConsensusOutputMisbehaviorCounts {
    pub faulty_blocks_provable: Vec<u64>,
    pub faulty_blocks_unprovable: Vec<u64>,
    pub missing_proposals: Vec<u64>,
    pub equivocations: Vec<u64>,
}

pub(crate) trait ConsensusOutputAPI: Display {
    fn reputation_score_sorted_desc(&self) -> Option<Vec<(AuthorityIndex, u64)>>;
    fn leader_round(&self) -> u64;
    fn leader_author_index(&self) -> AuthorityIndex;

    /// Returns epoch UNIX timestamp in milliseconds
    fn commit_timestamp_ms(&self) -> u64;

    /// Returns a unique global index for each committed sub-dag.
    fn commit_sub_dag_index(&self) -> u64;

    /// Returns all transactions in the commit.
    fn transactions(&self) -> ConsensusOutputTransactions;

    /// Returns the digest of consensus output.
    fn consensus_digest(&self) -> ConsensusCommitDigest;

    fn number_of_headers_in_commit_by_authority(&self) -> Vec<(AuthorityIndex, u64)>;

    /// Per-authority misbehavior counts observed in this commit. The default
    /// impl returns the all-empty struct so unwired implementations get
    /// correct behavior for free; the mapper zero-fills empty fields.
    fn misbehavior_counts(&self) -> ConsensusOutputMisbehaviorCounts {
        ConsensusOutputMisbehaviorCounts::default()
    }
}
impl ConsensusOutputAPI for starfish_core::CommittedSubDag {
    fn reputation_score_sorted_desc(&self) -> Option<Vec<(AuthorityIndex, u64)>> {
        if !self.reputation_scores_desc.is_empty() {
            Some(
                self.reputation_scores_desc
                    .iter()
                    .map(|(id, score)| (id.value() as AuthorityIndex, *score))
                    .collect(),
            )
        } else {
            None
        }
    }

    fn leader_round(&self) -> u64 {
        self.leader.round as u64
    }

    fn leader_author_index(&self) -> AuthorityIndex {
        self.leader.author.value() as AuthorityIndex
    }

    fn commit_timestamp_ms(&self) -> u64 {
        self.timestamp_ms
    }

    fn commit_sub_dag_index(&self) -> u64 {
        self.commit_ref.index.into()
    }

    fn transactions(&self) -> ConsensusOutputTransactions {
        self.transactions
            .iter()
            .map(|vt| {
                let round = vt.round() as u64;
                let author = vt.author().value() as AuthorityIndex;

                let transactions: Vec<_> = vt
                    .transactions()
                    .iter()
                    .flat_map(|tx| {
                        let transaction = bcs::from_bytes::<ConsensusTransaction>(tx.data());
                        match transaction {
                            Ok(transaction) => Some((transaction, tx.data().len())),
                            Err(err) => {
                                tracing::error!(
                                    "Failed to deserialize sequenced consensus transaction \
                                     (this should not happen) {err} from {author} at {round}"
                                );
                                None
                            }
                        }
                    })
                    .collect();

                (author, transactions)
            })
            .collect()
    }

    fn consensus_digest(&self) -> ConsensusCommitDigest {
        // Ensure wire layout matches.
        static_assertions::assert_eq_size!(ConsensusCommitDigest, starfish_core::CommitDigest);
        ConsensusCommitDigest::new(self.commit_ref.digest.into_inner())
    }

    fn number_of_headers_in_commit_by_authority(&self) -> Vec<(AuthorityIndex, u64)> {
        let mut num_of_committed_headers = BTreeMap::new();
        self.base
            .committed_header_refs
            .iter()
            .for_each(|block_ref| {
                let author_index = block_ref.author.value() as AuthorityIndex;
                *num_of_committed_headers.entry(author_index).or_insert(0) += 1;
            });
        num_of_committed_headers.into_iter().collect()
    }

    fn misbehavior_counts(&self) -> ConsensusOutputMisbehaviorCounts {
        let (faulty_blocks_provable, faulty_blocks_unprovable, missing_proposals, equivocations) =
            self.misbehavior_counts
                .iter()
                .map(|counts| match counts {
                    starfish_core::MisbehaviorCounts::V1(v1) => (
                        v1.faulty_blocks_provable,
                        v1.faulty_blocks_unprovable,
                        v1.missing_proposals,
                        v1.equivocations,
                    ),
                })
                .multiunzip();
        ConsensusOutputMisbehaviorCounts {
            faulty_blocks_provable,
            faulty_blocks_unprovable,
            missing_proposals,
            equivocations,
        }
    }
}

#[cfg(test)]
mod tests {
    use starfish_core::{
        BlockRef, CommitDigest, CommitRef, CommittedSubDag, MisbehaviorCounts, MisbehaviorCountsV1,
        VerifiedBlockHeader,
    };

    use super::*;

    #[test]
    fn test_misbehavior_counts_transposes_per_authority_to_per_field_vecs() {
        let counts = vec![
            MisbehaviorCounts::V1(MisbehaviorCountsV1 {
                faulty_blocks_provable: 1,
                faulty_blocks_unprovable: 2,
                missing_proposals: 3,
                equivocations: 4,
            }),
            MisbehaviorCounts::V1(MisbehaviorCountsV1 {
                faulty_blocks_provable: 10,
                faulty_blocks_unprovable: 20,
                missing_proposals: 30,
                equivocations: 40,
            }),
            MisbehaviorCounts::default(),
        ];

        let subdag = CommittedSubDag::new(
            BlockRef::MIN,
            Vec::<VerifiedBlockHeader>::new(),
            vec![],
            vec![],
            0,
            CommitRef::new(1, CommitDigest::MIN),
            vec![],
            counts,
        );

        let out = subdag.misbehavior_counts();
        assert_eq!(out.faulty_blocks_provable, vec![1, 10, 0]);
        assert_eq!(out.faulty_blocks_unprovable, vec![2, 20, 0]);
        assert_eq!(out.missing_proposals, vec![3, 30, 0]);
        assert_eq!(out.equivocations, vec![4, 40, 0]);
    }

    #[test]
    fn test_misbehavior_counts_empty_snapshot_produces_empty_vecs() {
        let subdag = CommittedSubDag::new(
            BlockRef::MIN,
            Vec::<VerifiedBlockHeader>::new(),
            vec![],
            vec![],
            0,
            CommitRef::new(1, CommitDigest::MIN),
            vec![],
            vec![],
        );
        let out = subdag.misbehavior_counts();
        assert!(out.faulty_blocks_provable.is_empty());
        assert!(out.faulty_blocks_unprovable.is_empty());
        assert!(out.missing_proposals.is_empty());
        assert!(out.equivocations.is_empty());
    }
}
