// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    ops::Bound::Included,
    sync::Arc,
};

use bytes::Bytes;
use parking_lot::RwLock;
use starfish_config::AuthorityIndex;

use super::{Store, WriteBatch};
use crate::{
    block_header::{
        BlockHeaderAPI as _, BlockHeaderDigest, BlockRef, Round, Slot, TransactionsCommitment,
        VerifiedBlock, VerifiedBlockHeader, VerifiedTransactions,
    },
    commit::{
        CommitAPI as _, CommitDigest, CommitIndex, CommitInfo, CommitRange, CommitRef,
        TrustedCommit,
    },
    context::Context,
    error::{ConsensusError, ConsensusResult},
    storage::rocksdb_store::check_ref_consistency,
    transaction_ref::{GenericTransactionRef, TransactionRef},
};

/// In-memory storage for testing.
pub(crate) struct MemStore {
    inner: RwLock<Inner>,
    context: Arc<Context>,
}

struct Inner {
    transactions: BTreeMap<(Round, AuthorityIndex, BlockHeaderDigest), VerifiedTransactions>,
    transactions_by_tx_refs:
        BTreeMap<(Round, AuthorityIndex, TransactionsCommitment), VerifiedTransactions>,
    block_headers: BTreeMap<(Round, AuthorityIndex, BlockHeaderDigest), VerifiedBlockHeader>,
    digests_by_authorities: BTreeSet<(AuthorityIndex, Round, BlockHeaderDigest)>,
    transaction_commitments_by_authorities:
        BTreeSet<(AuthorityIndex, Round, TransactionsCommitment)>,
    commits: BTreeMap<(CommitIndex, CommitDigest), TrustedCommit>,
    commit_votes: BTreeSet<(CommitIndex, CommitDigest, BlockRef)>,
    commit_info: BTreeMap<(CommitIndex, CommitDigest), CommitInfo>,
    /// Stores voting block headers separately from regular block headers.
    voting_block_headers: BTreeMap<(Round, AuthorityIndex, BlockHeaderDigest), VerifiedBlockHeader>,
    /// Flag indicating fast commit sync is ongoing.
    fast_sync_ongoing: bool,
}

impl MemStore {
    pub(crate) fn new(context: Arc<Context>) -> Self {
        MemStore {
            inner: RwLock::new(Inner {
                transactions: BTreeMap::new(),
                transactions_by_tx_refs: BTreeMap::new(),
                block_headers: BTreeMap::new(),
                digests_by_authorities: BTreeSet::new(),
                transaction_commitments_by_authorities: BTreeSet::new(),
                commits: BTreeMap::new(),
                commit_votes: BTreeSet::new(),
                commit_info: BTreeMap::new(),
                voting_block_headers: BTreeMap::new(),
                fast_sync_ongoing: false,
            }),
            context,
        }
    }
}

impl Store for MemStore {
    fn write(&self, write_batch: WriteBatch, context: Arc<Context>) -> ConsensusResult<()> {
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
            let transaction_ref = transaction.transaction_ref();
            if context.protocol_config.consensus_fast_commit_sync() {
                inner.transactions_by_tx_refs.insert(
                    (
                        transaction_ref.round,
                        transaction_ref.author,
                        transaction_ref.transactions_commitment,
                    ),
                    transaction,
                );

                inner.transaction_commitments_by_authorities.insert((
                    transaction_ref.author,
                    transaction_ref.round,
                    transaction_ref.transactions_commitment,
                ));
            } else {
                let block_ref = transaction
                    .block_ref()
                    .expect("block_ref must be present in non-transaction-ref path");
                inner.transactions.insert(
                    (block_ref.round, block_ref.author, block_ref.digest),
                    transaction,
                );
            }
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

        // Handle voting block headers
        for header in write_batch.voting_block_headers {
            let key = (header.round(), header.author(), header.digest());
            let block_ref = header.reference();
            // Store commit votes from this block header
            for vote in header.commit_votes() {
                inner
                    .commit_votes
                    .insert((vote.index, vote.digest, block_ref));
            }
            inner.voting_block_headers.insert(key, header);
        }

        if let Some(flag) = write_batch.fast_commit_sync_flag {
            inner.fast_sync_ongoing = flag;
        }

        Ok(())
    }

    fn read_verified_transactions(
        &self,
        refs: &[GenericTransactionRef],
    ) -> ConsensusResult<Vec<Option<VerifiedTransactions>>> {
        if !check_ref_consistency(refs) {
            return Err(ConsensusError::InconsistentTransactionRefVariants);
        }
        let inner = self.inner.read();
        let transactions = refs
            .iter()
            .map(|r| match r {
                GenericTransactionRef::BlockRef(b) => inner
                    .transactions
                    .get(&(b.round, b.author, b.digest))
                    .cloned(),
                GenericTransactionRef::TransactionRef(t) => inner
                    .transactions_by_tx_refs
                    .get(&(t.round, t.author, t.transactions_commitment))
                    .cloned(),
            })
            .collect();
        Ok(transactions)
    }

    fn read_serialized_transactions(
        &self,
        refs: &[GenericTransactionRef],
    ) -> ConsensusResult<Vec<Option<Bytes>>> {
        if !check_ref_consistency(refs) {
            return Err(ConsensusError::InconsistentTransactionRefVariants);
        }
        let inner = self.inner.read();
        let transactions = refs
            .iter()
            .map(|r| match r {
                GenericTransactionRef::BlockRef(b) => inner
                    .transactions
                    .get(&(b.round, b.author, b.digest))
                    .map(|tx| tx.serialized().clone()),
                GenericTransactionRef::TransactionRef(t) => inner
                    .transactions_by_tx_refs
                    .get(&(t.round, t.author, t.transactions_commitment))
                    .map(|tx| tx.serialized().clone()),
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
        let headers = self.read_verified_block_headers(refs)?;
        let tx_refs = if self.context.protocol_config.consensus_fast_commit_sync() {
            headers
                .iter()
                .map(|vh| {
                    if vh.is_none() {
                        GenericTransactionRef::TransactionRef(TransactionRef::default())
                    } else {
                        GenericTransactionRef::TransactionRef(
                            vh.as_ref().unwrap().transaction_ref(),
                        )
                    }
                })
                .collect::<Vec<GenericTransactionRef>>()
        } else {
            refs.iter()
                .map(|r| GenericTransactionRef::BlockRef(*r))
                .collect()
        };
        let transactions = self.read_verified_transactions(tx_refs.as_slice())?;
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

    fn contains_transactions(&self, refs: &[GenericTransactionRef]) -> ConsensusResult<Vec<bool>> {
        if !check_ref_consistency(refs) {
            return Err(ConsensusError::InconsistentTransactionRefVariants);
        }
        let inner = self.inner.read();
        let exist = refs
            .iter()
            .map(|r| match r {
                GenericTransactionRef::BlockRef(b) => inner
                    .transactions
                    .contains_key(&(b.round, b.author, b.digest)),
                GenericTransactionRef::TransactionRef(t) => inner
                    .transactions_by_tx_refs
                    .contains_key(&(t.round, t.author, t.transactions_commitment)),
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
                block.unwrap_or_else(|| panic!("Storage inconsistency: block {r:?} not found!")),
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
                block.unwrap_or_else(|| panic!("Storage inconsistency: block {r:?} not found!")),
            );
        }
        Ok(blocks)
    }

    fn read_verified_block_headers(
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

    fn read_serialized_block_headers(
        &self,
        refs: &[BlockRef],
    ) -> ConsensusResult<Vec<Option<Bytes>>> {
        let inner = self.inner.read();
        let serialized_headers = refs
            .iter()
            .map(|r| {
                inner
                    .block_headers
                    .get(&(r.round, r.author, r.digest))
                    .map(|header| header.serialized().clone())
            })
            .collect();
        Ok(serialized_headers)
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

    fn scan_block_references_by_author(
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

    fn scan_transaction_references_by_author(
        &self,
        author: AuthorityIndex,
        start_round: Round,
    ) -> ConsensusResult<Vec<TransactionRef>> {
        let inner = self.inner.read();
        let res = inner
            .transaction_commitments_by_authorities
            .range((
                Included((author, start_round, TransactionsCommitment::MIN)),
                Included((author, Round::MAX, TransactionsCommitment::MAX)),
            ))
            .map(|(author, round, commitment)| TransactionRef {
                round: *round,
                author: *author,
                transactions_commitment: *commitment,
            })
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
        if range.start() > range.end() {
            return Ok(vec![]);
        }
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

    fn read_highest_commit_index_with_votes(
        &self,
        up_to_index: CommitIndex,
    ) -> ConsensusResult<Option<CommitIndex>> {
        let inner = self.inner.read();
        // Do a reverse iteration to find the highest index with votes <= up_to_index
        let result = inner
            .commit_votes
            .range((
                Included((CommitIndex::MIN, CommitDigest::MIN, BlockRef::MIN)),
                Included((up_to_index, CommitDigest::MAX, BlockRef::MAX)),
            ))
            .next_back()
            .map(|(index, _, _)| *index);
        Ok(result)
    }

    fn read_lowest_commit_index_with_votes(
        &self,
        from_index: CommitIndex,
    ) -> ConsensusResult<Option<CommitIndex>> {
        let inner = self.inner.read();
        let result = inner
            .commit_votes
            .range((
                Included((from_index, CommitDigest::MIN, BlockRef::MIN)),
                std::ops::Bound::Unbounded,
            ))
            .next()
            .map(|(index, _, _)| *index);
        Ok(result)
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

    fn read_voting_block_headers(
        &self,
        refs: &[BlockRef],
    ) -> ConsensusResult<Vec<Option<VerifiedBlockHeader>>> {
        let inner = self.inner.read();
        let headers = refs
            .iter()
            .map(|r| {
                inner
                    .voting_block_headers
                    .get(&(r.round, r.author, r.digest))
                    .cloned()
            })
            .collect();
        Ok(headers)
    }

    fn read_fast_sync_ongoing(&self) -> bool {
        self.inner.read().fast_sync_ongoing
    }
}
