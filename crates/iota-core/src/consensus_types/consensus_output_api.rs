// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeMap, fmt::Display};

use iota_types::{digests::ConsensusCommitDigest, messages_consensus::ConsensusTransaction};

use crate::consensus_types::AuthorityIndex;
/// A list of tuples of:
/// (block origin authority index, all transactions contained in the block).
/// For each transaction, returns deserialized transaction and its serialized
/// size.
type ConsensusOutputTransactions = Vec<(AuthorityIndex, Vec<(ConsensusTransaction, usize)>)>;

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
}
