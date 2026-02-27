// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeMap, fmt::Display};

use consensus_core::BlockAPI;
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
macro_rules! impl_consensus_output_api {
    (
        // Type to implement for
        type = $ty:path,
        // Fully qualified commit digest type used in the size assertion
        commit_digest = $commit_digest:path,
        // How to iterate items that carry (round/author/txs)
        iterate = |$self_ident:ident, $item_ident:ident| $iter:expr,
        // How to read the round from an item (u64 or cast yourself later)
        round = |$round_item:ident| $round_expr:expr,
        // How to read the author index value (u32/usize; we cast to AuthorityIndex)
        author = |$auth_item:ident| $author_expr:expr,
        // How to get the `&[u8]` txs iterator source (something with `.iter()` over tx buffers)
        txs = |$txs_item:ident| $txs_expr:expr,
        // How to get the number of committed headers in the commit
        committed_header_refs = |$committed_header_refs_item:ident| $committed_header_refs_expr:expr
    ) => {
        impl ConsensusOutputAPI for $ty {
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
                let $self_ident = self;
                ($iter)
                    .map(|$item_ident| {
                        let round = { $round_expr } as u64;
                        let author = { $author_expr } as AuthorityIndex;

                        let transactions: Vec<_> = ({
                                let $txs_item = $item_ident;
                                $txs_expr
                            })
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
                static_assertions::assert_eq_size!(ConsensusCommitDigest, $commit_digest);
                ConsensusCommitDigest::new(self.commit_ref.digest.into_inner())
            }

            fn number_of_headers_in_commit_by_authority(&self) -> Vec<(AuthorityIndex, u64)> {
                let $self_ident = self;
                let mut num_of_committed_headers = BTreeMap::new();
                    $committed_header_refs_expr
                .iter()
                .for_each(|block_ref| {
                    let author_index = block_ref.author.value() as AuthorityIndex;
                    *num_of_committed_headers.entry(author_index).or_insert(0) += 1;
                });
                num_of_committed_headers.into_iter().collect()
            }
        }
    };
}

// ===== Use the macro for the two concrete types =====

// consensus_core::CommittedSubDag:
// - iterate over `self.blocks`
// - per-item accessors: round()/author().value()/transactions()
// - committed_header_refs: map blocks to BlockRef
impl_consensus_output_api! {
    type = consensus_core::CommittedSubDag,
    commit_digest = consensus_core::CommitDigest,
    iterate = |self_, block| self_.blocks.iter(),
    round   = |block| block.round(),
    author  = |block| block.author().value(),
    txs     = |block| block.transactions(),
    committed_header_refs = |self_| self_.blocks.iter().map(|b| b.reference()).collect::<Vec<_>>()
}

// starfish_core::CommittedSubDag:
// - iterate over `self.transactions` (VerifiedTransactions)
// - per-item accessors: round()/author().value()/transactions()
// - committed_header_refs: use committed_header_refs from SubDagBase
impl_consensus_output_api! {
    type = starfish_core::CommittedSubDag,
    commit_digest = starfish_core::CommitDigest,
    iterate = |self_, vt| self_.transactions.iter(),
    round   = |vt| vt.round(),
    author  = |vt| vt.author().value(),
    txs     = |vt| vt.transactions(),
    committed_header_refs = |self_| &self_.base.committed_header_refs
}
