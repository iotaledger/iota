// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[cfg(test)]
pub(crate) mod mem_store;
pub(crate) mod rocksdb_store;

#[cfg(test)]
mod store_tests;

use std::sync::Arc;

use bytes::Bytes;
use starfish_config::AuthorityIndex;

use crate::{
    CommitIndex,
    block_header::{BlockRef, Round, VerifiedBlock, VerifiedBlockHeader, VerifiedTransactions},
    commit::{CommitInfo, CommitRange, CommitRef, TrustedCommit},
    context::Context,
    error::ConsensusResult,
    transaction_ref::{GenericTransactionRef, TransactionRef},
};

/// A common interface for consensus storage.
pub(crate) trait Store: Send + Sync {
    /// Writes blocks, consensus commits and other data to store atomically.
    fn write(&self, write_batch: WriteBatch, context: Arc<Context>) -> ConsensusResult<()>;

    /// Reads complete blocks by combining transactions and headers for the
    /// given refs.
    #[cfg_attr(not(test), expect(dead_code))]
    fn read_blocks(&self, refs: &[BlockRef]) -> ConsensusResult<Vec<Option<VerifiedBlock>>>;

    /// Read and get verified block headers for the given refs.
    fn read_verified_block_headers(
        &self,
        refs: &[BlockRef],
    ) -> ConsensusResult<Vec<Option<VerifiedBlockHeader>>>;

    /// Read and get serialized block headers for the given refs.
    fn read_serialized_block_headers(
        &self,
        refs: &[BlockRef],
    ) -> ConsensusResult<Vec<Option<Bytes>>>;

    /// Read and get verified transactions for the given refs.
    fn read_verified_transactions(
        &self,
        refs: &[GenericTransactionRef],
    ) -> ConsensusResult<Vec<Option<VerifiedTransactions>>>;

    /// Read and get serialized transactions for the given refs.
    fn read_serialized_transactions(
        &self,
        refs: &[GenericTransactionRef],
    ) -> ConsensusResult<Vec<Option<Bytes>>>;

    /// Checks if transactions exist in the store.
    fn contains_transactions(&self, refs: &[GenericTransactionRef]) -> ConsensusResult<Vec<bool>>;

    /// Checks if block headers exist in the store.
    fn contains_block_headers(&self, refs: &[BlockRef]) -> ConsensusResult<Vec<bool>>;

    /// Checks whether there is any block at the given slot
    #[allow(dead_code)]
    fn contains_block_at_slot(&self, slot: crate::block_header::Slot) -> ConsensusResult<bool>;

    /// Reads blocks for an authority, from start_round.
    #[expect(dead_code)]
    fn scan_blocks_by_author(
        &self,
        authority: AuthorityIndex,
        start_round: Round,
    ) -> ConsensusResult<Vec<VerifiedBlock>>;

    // The method returns the last `num_of_rounds` rounds blocks by author in round
    // ascending order. When a `before_round` is defined then the blocks of
    // round `<=before_round` are returned. If not then the max value for round
    // will be used as cut off.
    #[cfg_attr(not(test), expect(dead_code))]
    fn scan_last_blocks_by_author(
        &self,
        author: AuthorityIndex,
        num_of_rounds: u64,
        before_round: Option<Round>,
    ) -> ConsensusResult<Vec<VerifiedBlock>>;

    fn scan_block_references_by_author(
        &self,
        author: AuthorityIndex,
        start_round: Round,
    ) -> ConsensusResult<Vec<BlockRef>>;

    fn scan_transaction_references_by_author(
        &self,
        author: AuthorityIndex,
        start_round: Round,
    ) -> ConsensusResult<Vec<TransactionRef>>;

    fn scan_transactions_by_author(
        &self,
        author: AuthorityIndex,
        start_round: Round,
        context: Arc<Context>,
    ) -> ConsensusResult<Vec<VerifiedTransactions>> {
        let refs = if context.protocol_config.consensus_fast_commit_sync() {
            self.scan_transaction_references_by_author(author, start_round)?
                .into_iter()
                .map(GenericTransactionRef::from)
                .collect::<Vec<_>>()
        } else {
            self.scan_block_references_by_author(author, start_round)?
                .into_iter()
                .map(GenericTransactionRef::from)
                .collect::<Vec<_>>()
        };
        Ok(self
            .read_verified_transactions(&refs)?
            .into_iter()
            .flatten()
            .collect())
    }
    fn scan_block_headers_by_author(
        &self,
        author: AuthorityIndex,
        start_round: Round,
    ) -> ConsensusResult<Vec<VerifiedBlockHeader>> {
        let refs = self.scan_block_references_by_author(author, start_round)?;
        let results = self.read_verified_block_headers(refs.as_slice())?;
        let mut block_headers = Vec::with_capacity(refs.len());
        for (r, block) in refs.into_iter().zip(results.into_iter()) {
            block_headers.push(
                block.unwrap_or_else(|| panic!("Storage inconsistency: block {r:?} not found!")),
            );
        }
        Ok(block_headers)
    }

    /// Reads the last commit.
    fn read_last_commit(&self) -> ConsensusResult<Option<TrustedCommit>>;

    /// Reads all commits from start (inclusive) until end (inclusive).
    fn scan_commits(&self, range: CommitRange) -> ConsensusResult<Vec<TrustedCommit>>;

    /// Reads all blocks voting on a particular commit.
    fn read_commit_votes(&self, commit_index: CommitIndex) -> ConsensusResult<Vec<BlockRef>>;

    /// Finds the highest commit index that has at least one vote, up to (and
    /// including) the given index. Returns None if no votes exist for any
    /// index <= up_to_index.
    fn read_highest_commit_index_with_votes(
        &self,
        up_to_index: CommitIndex,
    ) -> ConsensusResult<Option<CommitIndex>>;

    /// Finds the lowest commit index that has at least one vote, from (and
    /// including) the given index. Returns None if no votes exist for any
    /// index >= from_index.
    fn read_lowest_commit_index_with_votes(
        &self,
        from_index: CommitIndex,
    ) -> ConsensusResult<Option<CommitIndex>>;

    /// Reads the last commit info, written atomically with the last commit.
    fn read_last_commit_info(&self) -> ConsensusResult<Option<(CommitRef, CommitInfo)>>;

    /// Reads voting block headers from the separate voting storage.
    /// Returns None for headers that are not found.
    fn read_voting_block_headers(
        &self,
        refs: &[BlockRef],
    ) -> ConsensusResult<Vec<Option<VerifiedBlockHeader>>>;

    /// Returns true if fast commit sync was ongoing when the node last shut
    /// down.
    fn read_fast_sync_ongoing(&self) -> bool;
}

/// Represents data to be written to the store together atomically.
#[derive(Debug, Default)]
pub(crate) struct WriteBatch {
    pub(crate) transactions: Vec<VerifiedTransactions>,
    pub(crate) block_headers: Vec<VerifiedBlockHeader>,
    pub(crate) commits: Vec<TrustedCommit>,
    pub(crate) commit_info: Vec<(CommitRef, CommitInfo)>,
    pub(crate) voting_block_headers: Vec<VerifiedBlockHeader>,
    pub(crate) fast_commit_sync_flag: Option<bool>,
}

impl WriteBatch {
    pub(crate) fn new(
        transactions: Vec<VerifiedTransactions>,
        block_headers: Vec<VerifiedBlockHeader>,
        commits: Vec<TrustedCommit>,
        commit_info: Vec<(CommitRef, CommitInfo)>,
        voting_block_headers: Vec<VerifiedBlockHeader>,
        fast_commit_sync_flag: Option<bool>,
    ) -> Self {
        WriteBatch {
            transactions,
            block_headers,
            commits,
            commit_info,
            voting_block_headers,
            fast_commit_sync_flag,
        }
    }

    // Test setters.

    #[cfg(test)]
    pub(crate) fn transactions(mut self, transactions: Vec<VerifiedTransactions>) -> Self {
        self.transactions = transactions;
        self
    }

    #[cfg(test)]
    pub(crate) fn block_headers(mut self, block_headers: Vec<VerifiedBlockHeader>) -> Self {
        self.block_headers = block_headers;
        self
    }

    #[cfg(test)]
    pub(crate) fn commits(mut self, commits: Vec<TrustedCommit>) -> Self {
        self.commits = commits;
        self
    }

    #[cfg(test)]
    pub(crate) fn commit_info(mut self, commit_info: Vec<(CommitRef, CommitInfo)>) -> Self {
        self.commit_info = commit_info;
        self
    }

    #[cfg(test)]
    pub(crate) fn voting_block_headers(
        mut self,
        voting_block_headers: Vec<VerifiedBlockHeader>,
    ) -> Self {
        self.voting_block_headers = voting_block_headers;
        self
    }
}

/// Simulation-test-only helper that deletes all transactions from the consensus
/// RocksDB store while preserving other data.
#[cfg(msim)]
pub fn delete_all_transactions_from_store(
    db_path: &std::path::Path,
    authority_index: starfish_config::AuthorityIndex,
    committee: starfish_config::Committee,
    protocol_config: iota_protocol_config::ProtocolConfig,
) -> Result<(), String> {
    rocksdb_store::RocksDBStore::delete_all_transactions_from_store(
        db_path,
        authority_index,
        committee,
        protocol_config,
    )
    .map_err(|e| format!("failed to delete transactions: {}", e))
}
