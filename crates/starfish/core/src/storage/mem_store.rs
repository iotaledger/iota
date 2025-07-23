// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    ops::Bound::Included,
};

use parking_lot::RwLock;
use starfish_config::AuthorityIndex;

use super::{Store, WriteBatch};
use crate::{
    block_header::{
        BlockHeaderAPI as _, BlockHeaderDigest, BlockRef, Round, Slot, VerifiedBlock,
        VerifiedBlockHeader, VerifiedTransactions,
    },
    commit::{
        CommitAPI as _, CommitDigest, CommitIndex, CommitInfo, CommitRange, CommitRef,
        TrustedCommit,
    },
    error::ConsensusResult,
};

/// In-memory storage for testing.
pub(crate) struct MemStore {
    inner: RwLock<Inner>,
}

struct Inner {
    transactions: BTreeMap<(Round, AuthorityIndex, BlockHeaderDigest), VerifiedTransactions>,
    block_headers: BTreeMap<(Round, AuthorityIndex, BlockHeaderDigest), VerifiedBlockHeader>,
    digests_by_authorities: BTreeSet<(AuthorityIndex, Round, BlockHeaderDigest)>,
    commits: BTreeMap<(CommitIndex, CommitDigest), TrustedCommit>,
    commit_votes: BTreeSet<(CommitIndex, CommitDigest, BlockRef)>,
    commit_info: BTreeMap<(CommitIndex, CommitDigest), CommitInfo>,
}

impl MemStore {
    pub(crate) fn new() -> Self {
        MemStore {
            inner: RwLock::new(Inner {
                transactions: BTreeMap::new(),
                block_headers: BTreeMap::new(),
                digests_by_authorities: BTreeSet::new(),
                commits: BTreeMap::new(),
                commit_votes: BTreeSet::new(),
                commit_info: BTreeMap::new(),
            }),
        }
    }
}

impl Store for MemStore {
    fn write(&self, write_batch: WriteBatch) -> ConsensusResult<()> {
        let mut inner = self.inner.write();

        // Store block headers
        for block_header in write_batch.block_headers {
            let block_ref = block_header.reference();
            inner.block_headers.insert(
                (block_ref.round, block_ref.author, block_ref.digest),
                block_header.clone(),
            );
            inner.digests_by_authorities.insert((
                block_ref.author,
                block_ref.round,
                block_ref.digest,
            ));
            for vote in block_header.commit_votes() {
                inner
                    .commit_votes
                    .insert((vote.index, vote.digest, block_ref));
            }
        }

        // Store transactions data separately
        for transaction in write_batch.transactions {
            let block_ref = transaction.block_ref();
            inner.transactions.insert(
                (block_ref.round, block_ref.author, block_ref.digest),
                transaction,
            );
        }

        for commit in write_batch.commits {
            inner
                .commits
                .insert((commit.index(), commit.digest()), commit);
        }

        for (commit_ref, commit_info) in write_batch.commit_info {
            inner
                .commit_info
                .insert((commit_ref.index, commit_ref.digest), commit_info);
        }

        Ok(())
    }

    fn read_transactions(
        &self,
        refs: &[BlockRef],
    ) -> ConsensusResult<Vec<Option<VerifiedTransactions>>> {
        let inner = self.inner.read();
        let transactions = refs
            .iter()
            .map(|r| {
                inner
                    .transactions
                    .get(&(r.round, r.author, r.digest))
                    .cloned()
            })
            .collect();
        Ok(transactions)
    }

    // TODO: Do we need this method or will DAGState always try to read both headers
    // and transactions separately?
    fn read_blocks(&self, refs: &[BlockRef]) -> ConsensusResult<Vec<Option<VerifiedBlock>>> {
        // Ensure we have a read lock on the inner state across reading both headers and
        // transactions reads
        let inner = self.inner.read();
        // Get both headers and transactions for the given references
        let headers = self.read_block_headers(refs)?;
        let transactions = self.read_transactions(refs)?;
        drop(inner); // Explicitly drop the read lock before combining results

        // Combine them into blocks if both parts exist
        let mut blocks = Vec::with_capacity(refs.len());
        for (header, transactions) in headers.into_iter().zip(transactions) {
            match (header, transactions) {
                (Some(hdr), Some(txs)) => {
                    blocks.push(Some(VerifiedBlock::new(hdr, txs)));
                }
                _ => blocks.push(None),
            }
        }
        Ok(blocks)
    }

    fn contains_transactions(&self, refs: &[BlockRef]) -> ConsensusResult<Vec<bool>> {
        let inner = self.inner.read();
        let exist = refs
            .iter()
            .map(|r| {
                inner
                    .transactions
                    .contains_key(&(r.round, r.author, r.digest))
            })
            .collect();
        Ok(exist)
    }

    fn scan_blocks_by_author(
        &self,
        author: AuthorityIndex,
        start_round: Round,
    ) -> ConsensusResult<Vec<VerifiedBlock>> {
        let inner = self.inner.read();
        let mut refs = vec![];
        for &(author, round, digest) in inner.digests_by_authorities.range((
            Included((author, start_round, BlockHeaderDigest::MIN)),
            Included((author, Round::MAX, BlockHeaderDigest::MAX)),
        )) {
            refs.push(BlockRef::new(round, author, digest));
        }
        let results = self.read_blocks(refs.as_slice())?;
        let mut blocks = Vec::with_capacity(refs.len());
        for (r, block) in refs.into_iter().zip(results.into_iter()) {
            blocks.push(
                block.unwrap_or_else(|| panic!("Storage inconsistency: block {:?} not found!", r)),
            );
        }
        Ok(blocks)
    }

    fn scan_last_blocks_by_author(
        &self,
        author: AuthorityIndex,
        num_of_rounds: u64,
        before_round: Option<Round>,
    ) -> ConsensusResult<Vec<VerifiedBlock>> {
        let before_round = before_round.unwrap_or(Round::MAX);
        let mut refs = VecDeque::new();

        // Collect block references
        for &(author, round, digest) in self
            .inner
            .read()
            .digests_by_authorities
            .range((
                Included((author, Round::MIN, BlockHeaderDigest::MIN)),
                Included((author, before_round, BlockHeaderDigest::MAX)),
            ))
            .rev()
            .take(num_of_rounds as usize)
        {
            refs.push_front(BlockRef::new(round, author, digest));
        }

        // Read and combine transactions and headers
        let results = self.read_blocks(refs.as_slices().0)?;
        let mut blocks = vec![];
        for (r, block) in refs.into_iter().zip(results.into_iter()) {
            blocks.push(
                block.unwrap_or_else(|| panic!("Storage inconsistency: block {:?} not found!", r)),
            );
        }
        Ok(blocks)
    }

    fn read_block_headers(
        &self,
        refs: &[BlockRef],
    ) -> ConsensusResult<Vec<Option<VerifiedBlockHeader>>> {
        let inner = self.inner.read();
        let block_headers = refs
            .iter()
            .map(|r| {
                inner
                    .block_headers
                    .get(&(r.round, r.author, r.digest))
                    .cloned()
            })
            .collect();
        Ok(block_headers)
    }

    fn contains_block_at_slot(&self, slot: Slot) -> ConsensusResult<bool> {
        let inner = self.inner.read();
        let found = inner
            .digests_by_authorities
            .range((
                Included((slot.authority, slot.round, BlockHeaderDigest::MIN)),
                Included((slot.authority, slot.round, BlockHeaderDigest::MAX)),
            ))
            .next()
            .is_some();
        Ok(found)
    }

    fn scan_references_by_author(
        &self,
        author: AuthorityIndex,
        start_round: Round,
    ) -> ConsensusResult<Vec<BlockRef>> {
        let inner = self.inner.read();
        let res = inner
            .digests_by_authorities
            .range((
                Included((author, start_round, BlockHeaderDigest::MIN)),
                Included((author, Round::MAX, BlockHeaderDigest::MAX)),
            ))
            .map(|(author, round, digest)| BlockRef::new(*round, *author, *digest))
            .collect();
        Ok(res)
    }

    fn read_last_commit(&self) -> ConsensusResult<Option<TrustedCommit>> {
        let inner = self.inner.read();
        Ok(inner
            .commits
            .iter()
            .next_back()
            .map(|(_, commit)| commit.clone()))
    }

    fn scan_commits(&self, range: CommitRange) -> ConsensusResult<Vec<TrustedCommit>> {
        let inner = self.inner.read();
        let mut commits = vec![];
        for (_, commit) in inner.commits.range((
            Included((range.start(), CommitDigest::MIN)),
            Included((range.end(), CommitDigest::MAX)),
        )) {
            commits.push(commit.clone());
        }
        Ok(commits)
    }

    fn read_commit_votes(&self, commit_index: CommitIndex) -> ConsensusResult<Vec<BlockRef>> {
        let inner = self.inner.read();
        let votes = inner
            .commit_votes
            .range((
                Included((commit_index, CommitDigest::MIN, BlockRef::MIN)),
                Included((commit_index, CommitDigest::MAX, BlockRef::MAX)),
            ))
            .map(|(_, _, block_ref)| *block_ref)
            .collect();
        Ok(votes)
    }

    fn read_last_commit_info(&self) -> ConsensusResult<Option<(CommitRef, CommitInfo)>> {
        let inner = self.inner.read();
        Ok(inner
            .commit_info
            .iter()
            .next_back()
            .map(|((index, digest), info)| (CommitRef::new(*index, *digest), info.clone())))
    }

    fn contains_block_headers(&self, refs: &[BlockRef]) -> ConsensusResult<Vec<bool>> {
        let inner = self.inner.read();
        let exist = refs
            .iter()
            .map(|r| {
                inner
                    .block_headers
                    .contains_key(&(r.round, r.author, r.digest))
            })
            .collect();
        Ok(exist)
    }
}
