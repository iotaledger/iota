// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{ops::Bound::Included, sync::Arc, time::Duration};

use bytes::Bytes;
use iota_macros::fail_point;
use starfish_config::AuthorityIndex;
use tracing::debug;
use typed_store::{
    Map as _,
    metrics::SamplingInterval,
    reopen,
    rocks::{DBMap, MetricConf, ReadWriteOptions, default_db_options, open_cf_opts},
};

use super::{CommitInfo, Store, WriteBatch};
use crate::{
    Transaction,
    block_header::{
        BlockHeaderAPI as _, BlockHeaderDigest, BlockRef, GenericTransactionRef, Round,
        SignedBlockHeader, TransactionRef, TransactionsCommitment, VerifiedBlock,
        VerifiedBlockHeader, VerifiedTransactions,
    },
    commit::{CommitAPI as _, CommitDigest, CommitIndex, CommitRange, CommitRef, TrustedCommit},
    context::Context,
    error::{ConsensusError, ConsensusResult},
};

/// Persistent storage with RocksDB.
pub(crate) struct RocksDBStore {
    /// Stores SignedBlockHeader by refs.
    block_headers: DBMap<(Round, AuthorityIndex, BlockHeaderDigest), Bytes>,
    /// Stores Transactions by block refs
    transactions: DBMap<(Round, AuthorityIndex, BlockHeaderDigest), Bytes>,
    /// Stores Transactions by transaction refs
    transactions_by_tr_refs: DBMap<(Round, AuthorityIndex, TransactionsCommitment), Bytes>,
    /// A secondary index that orders refs first by authors.
    digests_by_authorities: DBMap<(AuthorityIndex, Round, BlockHeaderDigest), ()>,
    /// A secondary index that orders refs first by authors.
    transaction_commitments_by_authorities: DBMap<
        (
            AuthorityIndex,
            Round,
            TransactionsCommitment,
            BlockHeaderDigest,
        ),
        (),
    >,
    /// Maps commit index to Commit.
    commits: DBMap<(CommitIndex, CommitDigest), Bytes>,
    /// Collects votes on commits.
    /// TODO: batch multiple votes into a single row.
    commit_votes: DBMap<(CommitIndex, CommitDigest, BlockRef), ()>,
    /// Stores info related to Commit that helps recovery.
    commit_info: DBMap<(CommitIndex, CommitDigest), CommitInfo>,
    /// Context to access protocol configuration
    context: Arc<Context>,
}

impl RocksDBStore {
    const TRANSACTIONS_CF: &'static str = "transactions";
    const TRANSACTIONS_BY_TR_REF_CF: &'static str = "transactions_by_tr_refs";
    const BLOCK_HEADERS_CF: &'static str = "block_headers";
    const DIGESTS_BY_AUTHORITIES_CF: &'static str = "digests";
    const TRANSACTION_COMMITMENTS_BY_AUTHORITIES_CF: &'static str =
        "transaction_commitments_by_authorities";
    const COMMITS_CF: &'static str = "commits";
    const COMMIT_VOTES_CF: &'static str = "commit_votes";
    const COMMIT_INFO_CF: &'static str = "commit_info";

    /// Creates a new instance of RocksDB storage.
    pub(crate) fn new(path: &str, context: Arc<Context>) -> Self {
        // Consensus data has high write throughput (all transactions) and is rarely
        // read (only during recovery and when helping peers catch up).
        let db_options = default_db_options().optimize_db_for_write_throughput(2);
        let mut metrics_conf = MetricConf::new("consensus");
        metrics_conf.read_sample_interval = SamplingInterval::new(Duration::from_secs(60), 0);
        let cf_options = default_db_options().optimize_for_write_throughput().options;
        let column_family_options = vec![
            (
                Self::TRANSACTIONS_CF,
                default_db_options()
                    .optimize_for_write_throughput_no_deletion()
                    // Using larger block is ok since there is not much point reads on the cf.
                    .set_block_options(512, 128 << 10)
                    .options,
            ),
            (
                Self::TRANSACTIONS_BY_TR_REF_CF,
                default_db_options()
                    .optimize_for_write_throughput_no_deletion()
                    // Using larger block is ok since there is not much point reads on the cf.
                    .set_block_options(512, 128 << 10)
                    .options,
            ),
            (
                Self::BLOCK_HEADERS_CF,
                default_db_options()
                    .optimize_for_write_throughput_no_deletion()
                    // TODO:think about these constants, for now it is a copy from blocks
                    .set_block_options(512, 128 << 10)
                    .options,
            ),
            (Self::DIGESTS_BY_AUTHORITIES_CF, cf_options.clone()),
            (
                Self::TRANSACTION_COMMITMENTS_BY_AUTHORITIES_CF,
                cf_options.clone(),
            ),
            (Self::COMMITS_CF, cf_options.clone()),
            (Self::COMMIT_VOTES_CF, cf_options.clone()),
            (Self::COMMIT_INFO_CF, cf_options.clone()),
        ];
        let rocksdb = open_cf_opts(
            path,
            Some(db_options.options),
            metrics_conf,
            &column_family_options,
        )
        .expect("Cannot open database");

        let (
            block_headers,
            transactions,
            transactions_by_tr_refs,
            digests_by_authorities,
            transaction_commitments_by_authorities,
            commits,
            commit_votes,
            commit_info,
        ) = reopen!(&rocksdb,
            Self::BLOCK_HEADERS_CF;<(Round, AuthorityIndex, BlockHeaderDigest), Bytes>,
            Self::TRANSACTIONS_CF;<(Round, AuthorityIndex, BlockHeaderDigest), Bytes>,
            Self::TRANSACTIONS_BY_TR_REF_CF;<(Round, AuthorityIndex, TransactionsCommitment), Bytes>,
            Self::DIGESTS_BY_AUTHORITIES_CF;<(AuthorityIndex, Round, BlockHeaderDigest), ()>,
            Self::TRANSACTION_COMMITMENTS_BY_AUTHORITIES_CF;<(AuthorityIndex, Round, TransactionsCommitment, BlockHeaderDigest), ()>,
            Self::COMMITS_CF;<(CommitIndex, CommitDigest), Bytes>,
            Self::COMMIT_VOTES_CF;<(CommitIndex, CommitDigest, BlockRef), ()>,
            Self::COMMIT_INFO_CF;<(CommitIndex, CommitDigest), CommitInfo>
        );

        Self {
            block_headers,
            transactions,
            transactions_by_tr_refs,
            digests_by_authorities,
            transaction_commitments_by_authorities,
            commits,
            commit_votes,
            commit_info,
            context,
        }
    }
}

impl Store for RocksDBStore {
    fn write(&self, write_batch: WriteBatch, context: Arc<Context>) -> ConsensusResult<()> {
        fail_point!("consensus-store-before-write");
        // TODO: does it matter which CF we use here?
        let mut batch = self.block_headers.batch();

        // Store block headers and their associated commit votes
        for block_header in write_batch.block_headers {
            let block_ref = block_header.reference();
            debug!("block header {} pushed to store", block_header);
            // Store the block header
            batch
                .insert_batch(
                    &self.block_headers,
                    [(
                        (block_ref.round, block_ref.author, block_ref.digest),
                        block_header.serialized(),
                    )],
                )
                .map_err(ConsensusError::RocksDBFailure)?;
            // Store the authority digest
            batch
                .insert_batch(
                    &self.digests_by_authorities,
                    [((block_ref.author, block_ref.round, block_ref.digest), ())],
                )
                .map_err(ConsensusError::RocksDBFailure)?;
            // Store commit votes from this block header using the BlockHeaderAPI trait
            for vote in block_header.commit_votes() {
                batch
                    .insert_batch(
                        &self.commit_votes,
                        [((vote.index, vote.digest, block_ref), ())],
                    )
                    .map_err(ConsensusError::RocksDBFailure)?;
            }
        }

        // Store transactions data
        for transaction in write_batch.transactions {
            let transaction_ref = transaction.transaction_ref();
            if context.protocol_config.consensus_transaction_ref() {
                batch
                    .insert_batch(
                        &self.transactions_by_tr_refs,
                        [(
                            (
                                transaction_ref.round,
                                transaction_ref.author,
                                transaction_ref.transactions_commitment,
                            ),
                            transaction.serialized(),
                        )],
                    )
                    .map_err(ConsensusError::RocksDBFailure)?;
                // Store the authority digest
                batch
                    .insert_batch(
                        &self.transaction_commitments_by_authorities,
                        [(
                            (
                                transaction_ref.author,
                                transaction_ref.round,
                                transaction_ref.transactions_commitment,
                                transaction_ref.block_digest,
                            ),
                            (),
                        )],
                    )
                    .map_err(ConsensusError::RocksDBFailure)?;
            } else {
                batch
                    .insert_batch(
                        &self.transactions,
                        [(
                            (
                                transaction_ref.round,
                                transaction_ref.author,
                                transaction_ref.block_digest,
                            ),
                            transaction.serialized(),
                        )],
                    )
                    .map_err(ConsensusError::RocksDBFailure)?;
            }
        }

        // Handle commits
        for commit in write_batch.commits {
            batch
                .insert_batch(
                    &self.commits,
                    [((commit.index(), commit.digest()), commit.serialized())],
                )
                .map_err(ConsensusError::RocksDBFailure)?;
        }

        // Handle commit info
        for (commit_ref, commit_info) in write_batch.commit_info {
            batch
                .insert_batch(
                    &self.commit_info,
                    [((commit_ref.index, commit_ref.digest), commit_info)],
                )
                .map_err(ConsensusError::RocksDBFailure)?;
        }

        batch.write()?;
        fail_point!("consensus-store-after-write");
        Ok(())
    }

    fn read_blocks(&self, refs: &[BlockRef]) -> ConsensusResult<Vec<Option<VerifiedBlock>>> {
        // Get both headers and transactions for the given references
        let headers = self.read_verified_block_headers(refs)?;
        let tr_refs = if self.context.protocol_config.consensus_transaction_ref() {
            headers
                .iter()
                .map(|vh| {
                    if vh.is_none() {
                        return GenericTransactionRef::TransactionRef(TransactionRef::default());
                    } else {
                        GenericTransactionRef::TransactionRef(
                            vh.as_ref().unwrap().transaction_ref(),
                        )
                    }
                })
                .collect::<Vec<GenericTransactionRef>>()
        } else {
            refs.iter()
                .map(|r| GenericTransactionRef::BlockRef(r.clone()))
                .collect::<Vec<GenericTransactionRef>>()
        };
        let transactions = self.read_verified_transactions(tr_refs.as_slice())?;

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

    /// Return Verified Block Headers by reading their entries from storage and
    /// deserializing them
    fn read_verified_block_headers(
        &self,
        refs: &[BlockRef],
    ) -> ConsensusResult<Vec<Option<VerifiedBlockHeader>>> {
        let serialized_block_headers = self.read_serialized_block_headers(refs)?;
        let mut block_headers = vec![];
        for (key, serialized_block_header) in refs.iter().zip(serialized_block_headers) {
            if let Some(serialized_block_header) = serialized_block_header {
                let block_header = VerifiedBlockHeader::new_from_bytes(serialized_block_header)?;

                // Makes sure block data is not corrupted by comparing digests.
                assert_eq!(*key, block_header.reference());
                block_headers.push(Some(block_header));
            } else {
                block_headers.push(None);
            }
        }
        Ok(block_headers)
    }

    /// Return Bytes of Block Headers by reading their entries from storage
    fn read_serialized_block_headers(
        &self,
        refs: &[BlockRef],
    ) -> ConsensusResult<Vec<Option<Bytes>>> {
        let keys = refs
            .iter()
            .map(|r| (r.round, r.author, r.digest))
            .collect::<Vec<_>>();
        let serialized_block_headers = self.block_headers.multi_get(keys)?;
        Ok(serialized_block_headers)
    }

    /// Return Verified Transactions by reading both transactions and headers
    /// from storage, deserializing them and assembling into the required output
    fn read_verified_transactions(
        &self,
        refs: &[GenericTransactionRef],
    ) -> ConsensusResult<Vec<Option<VerifiedTransactions>>> {
        if !check_ref_consistency(refs) {
            return Err(ConsensusError::InconsistentTransactionRefVariants);
        }
        let serialized_vec_transactions = self.read_serialized_transactions(refs)?;
        if refs.is_empty() {
            return Ok(vec![]);
        }
        let transaction_ref_enabled = match &refs[0] {
            GenericTransactionRef::BlockRef { .. } => false,
            GenericTransactionRef::TransactionRef { .. } => true,
        };
        let mut result = Vec::with_capacity(refs.len());
        if transaction_ref_enabled {
            for (gen_tr_ref, serialized_transactions) in
                refs.iter().zip(serialized_vec_transactions)
            {
                let GenericTransactionRef::TransactionRef(tr_ref) = gen_tr_ref else {
                    return Err(ConsensusError::InconsistentTransactionRefVariants);
                };
                let block_ref = BlockRef {
                    author: tr_ref.author,
                    round: tr_ref.round,
                    digest: tr_ref.block_digest,
                };
                if let Some(serialized_transactions) = serialized_transactions {
                    let transactions: Vec<Transaction> = bcs::from_bytes(&serialized_transactions)
                        .map_err(ConsensusError::MalformedTransactions)?;
                    // We don't check the transactions commitment from the header as it's loaded
                    // from storage. Assemble verified transactions
                    let verified_transactions = VerifiedTransactions::new(
                        transactions,
                        block_ref,
                        tr_ref.transactions_commitment,
                        serialized_transactions,
                    );
                    result.push(Some(verified_transactions));
                } else {
                    result.push(None);
                }
            }
        } else {
            let block_refs = refs
                .iter()
                .map(|gen_tr_ref| {
                    let GenericTransactionRef::BlockRef(block_ref) = gen_tr_ref else {
                        return Err(ConsensusError::InconsistentTransactionRefVariants);
                    };
                    Ok(*block_ref)
                })
                .collect::<Result<Vec<_>, ConsensusError>>()?;
            let serialized_block_headers =
                self.read_serialized_block_headers(block_refs.as_slice())?;

            // TODO::optimize it later by storing commitment together with transactions

            for ((block_ref, serialized_block_header), serialized_transactions) in block_refs
                .iter()
                .zip(serialized_block_headers)
                .zip(serialized_vec_transactions)
            {
                if let (Some(serialized_block_header), Some(serialized_transactions)) =
                    (serialized_block_header, serialized_transactions)
                {
                    let signed_block_header: SignedBlockHeader =
                        bcs::from_bytes(&serialized_block_header)
                            .map_err(ConsensusError::MalformedHeader)?;
                    let transactions: Vec<Transaction> = bcs::from_bytes(&serialized_transactions)
                        .map_err(ConsensusError::MalformedTransactions)?;
                    // We don't check the transactions commitment from the header as it's loaded
                    // from storage. Assemble verified transactions
                    let verified_transactions = VerifiedTransactions::new(
                        transactions,
                        *block_ref,
                        signed_block_header.transactions_commitment(),
                        serialized_transactions,
                    );
                    result.push(Some(verified_transactions));
                } else {
                    result.push(None);
                }
            }
        }
        Ok(result)
    }

    /// Return Bytes of corresponding Transactions by reading their entries from
    /// storage
    fn read_serialized_transactions(
        &self,
        refs: &[GenericTransactionRef],
    ) -> ConsensusResult<Vec<Option<Bytes>>> {
        if !check_ref_consistency(refs) {
            return Err(ConsensusError::InconsistentTransactionRefVariants);
        }
        if refs.is_empty() {
            return Ok(vec![]);
        }
        match &refs[0] {
            GenericTransactionRef::BlockRef { .. } => {
                let keys: Result<Vec<_>, ConsensusError> = refs
                    .iter()
                    .map(|r| {
                        if let GenericTransactionRef::BlockRef(block_ref) = r {
                            Ok((block_ref.round, block_ref.author, block_ref.digest))
                        } else {
                            Err(ConsensusError::InconsistentTransactionRefVariants)
                        }
                    })
                    .collect();
                Ok(self.transactions.multi_get(keys?)?)
            }
            GenericTransactionRef::TransactionRef { .. } => {
                let keys: Result<Vec<_>, ConsensusError> = refs
                    .iter()
                    .map(|r| {
                        if let GenericTransactionRef::TransactionRef(tr_ref) = r {
                            Ok((tr_ref.round, tr_ref.author, tr_ref.transactions_commitment))
                        } else {
                            Err(ConsensusError::InconsistentTransactionRefVariants)
                        }
                    })
                    .collect();
                Ok(self.transactions_by_tr_refs.multi_get(keys?)?)
            }
        }
    }

    fn contains_transactions(&self, refs: &[GenericTransactionRef]) -> ConsensusResult<Vec<bool>> {
        if !check_ref_consistency(refs) {
            return Err(ConsensusError::InconsistentTransactionRefVariants);
        }
        if refs.is_empty() {
            return Ok(vec![]);
        }
        match &refs[0] {
            GenericTransactionRef::BlockRef { .. } => {
                let keys: Result<Vec<_>, ConsensusError> = refs
                    .iter()
                    .map(|r| {
                        if let GenericTransactionRef::BlockRef(block_ref) = r {
                            Ok((block_ref.round, block_ref.author, block_ref.digest))
                        } else {
                            Err(ConsensusError::InconsistentTransactionRefVariants)
                        }
                    })
                    .collect();
                Ok(self.transactions.multi_contains_keys(keys?)?)
            }
            GenericTransactionRef::TransactionRef { .. } => {
                let keys: Result<Vec<_>, ConsensusError> = refs
                    .iter()
                    .map(|r| {
                        if let GenericTransactionRef::TransactionRef(tr_ref) = r {
                            Ok((tr_ref.round, tr_ref.author, tr_ref.transactions_commitment))
                        } else {
                            Err(ConsensusError::InconsistentTransactionRefVariants)
                        }
                    })
                    .collect();
                Ok(self.transactions_by_tr_refs.multi_contains_keys(keys?)?)
            }
        }
    }

    fn contains_block_headers(&self, refs: &[BlockRef]) -> ConsensusResult<Vec<bool>> {
        let refs = refs
            .iter()
            .map(|r| (r.round, r.author, r.digest))
            .collect::<Vec<_>>();
        let exist = self.block_headers.multi_contains_keys(refs)?;
        Ok(exist)
    }

    fn contains_block_at_slot(&self, slot: crate::block_header::Slot) -> ConsensusResult<bool> {
        let found = self
            .digests_by_authorities
            .safe_range_iter((
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
        self.digests_by_authorities
            .safe_range_iter((
                Included((author, start_round, BlockHeaderDigest::MIN)),
                Included((author, Round::MAX, BlockHeaderDigest::MAX)),
            ))
            .map(|res| {
                let ((author, round, digest), _) = res?;
                Ok(BlockRef::new(round, author, digest))
            })
            .collect()
    }

    fn scan_transaction_references_by_author(
        &self,
        author: AuthorityIndex,
        start_round: Round,
    ) -> ConsensusResult<Vec<TransactionRef>> {
        self.transaction_commitments_by_authorities
            .safe_range_iter((
                Included((
                    author,
                    start_round,
                    TransactionsCommitment::MIN,
                    BlockHeaderDigest::MIN,
                )),
                Included((
                    author,
                    Round::MAX,
                    TransactionsCommitment::MAX,
                    BlockHeaderDigest::MAX,
                )),
            ))
            .map(|res| {
                let ((author, round, commitment, digest), _) = res?;
                Ok(TransactionRef {
                    round,
                    author,
                    transactions_commitment: commitment,
                    block_digest: digest,
                })
            })
            .collect()
    }

    fn scan_blocks_by_author(
        &self,
        author: AuthorityIndex,
        start_round: Round,
    ) -> ConsensusResult<Vec<VerifiedBlock>> {
        let mut refs = vec![];
        for kv in self.digests_by_authorities.safe_range_iter((
            Included((author, start_round, BlockHeaderDigest::MIN)),
            Included((author, Round::MAX, BlockHeaderDigest::MAX)),
        )) {
            let ((author, round, digest), _) = kv?;
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

    // The method returns the last `num_of_rounds` rounds blocks by author in round
    // ascending order. When a `before_round` is defined then the blocks of
    // round `<=before_round` are returned. If not then the max value for round
    // will be used as cut off.
    fn scan_last_blocks_by_author(
        &self,
        author: AuthorityIndex,
        num_of_rounds: u64,
        before_round: Option<Round>,
    ) -> ConsensusResult<Vec<VerifiedBlock>> {
        let before_round = before_round.unwrap_or(Round::MAX);
        let mut refs = std::collections::VecDeque::new();
        for kv in self
            .digests_by_authorities
            .reversed_safe_iter_with_bounds(
                Some((author, Round::MIN, BlockHeaderDigest::MIN)),
                Some((author, before_round, BlockHeaderDigest::MAX)),
            )?
            .take(num_of_rounds as usize)
        {
            let ((author, round, digest), _) = kv?;
            refs.push_front(BlockRef::new(round, author, digest));
        }
        let results = self.read_blocks(refs.as_slices().0)?;
        let mut blocks = vec![];
        for (r, block) in refs.into_iter().zip(results.into_iter()) {
            blocks.push(
                block.unwrap_or_else(|| panic!("Storage inconsistency: block {r:?} not found!")),
            );
        }
        Ok(blocks)
    }

    fn read_last_commit(&self) -> ConsensusResult<Option<TrustedCommit>> {
        let Some(result) = self
            .commits
            .reversed_safe_iter_with_bounds(None, None)?
            .next()
        else {
            return Ok(None);
        };
        let ((_index, digest), serialized) = result?;
        let commit = TrustedCommit::new_trusted(
            bcs::from_bytes(&serialized).map_err(ConsensusError::MalformedCommit)?,
            serialized,
        );
        assert_eq!(commit.digest(), digest);
        Ok(Some(commit))
    }

    fn scan_commits(&self, range: CommitRange) -> ConsensusResult<Vec<TrustedCommit>> {
        let mut commits = vec![];
        for result in self.commits.safe_range_iter((
            Included((range.start(), CommitDigest::MIN)),
            Included((range.end(), CommitDigest::MAX)),
        )) {
            let ((_index, digest), serialized) = result?;
            let commit = TrustedCommit::new_trusted(
                bcs::from_bytes(&serialized).map_err(ConsensusError::MalformedCommit)?,
                serialized,
            );
            assert_eq!(commit.digest(), digest);
            commits.push(commit);
        }
        Ok(commits)
    }

    fn read_commit_votes(&self, commit_index: CommitIndex) -> ConsensusResult<Vec<BlockRef>> {
        let mut votes = Vec::new();
        for vote in self.commit_votes.safe_range_iter((
            Included((commit_index, CommitDigest::MIN, BlockRef::MIN)),
            Included((commit_index, CommitDigest::MAX, BlockRef::MAX)),
        )) {
            let ((_, _, block_ref), _) = vote?;
            votes.push(block_ref);
        }
        Ok(votes)
    }

    fn read_last_commit_info(&self) -> ConsensusResult<Option<(CommitRef, CommitInfo)>> {
        let Some(result) = self
            .commit_info
            .reversed_safe_iter_with_bounds(None, None)?
            .next()
        else {
            return Ok(None);
        };
        let (key, commit_info) = result.map_err(ConsensusError::RocksDBFailure)?;
        Ok(Some((CommitRef::new(key.0, key.1), commit_info)))
    }
}

/// Returns true if all elements in the slice have the same variant (by
/// discriminant)
pub(crate) fn check_ref_consistency(refs: &[GenericTransactionRef]) -> bool {
    if refs.is_empty() {
        return true;
    }
    let first = std::mem::discriminant(&refs[0]);
    refs.iter().all(|r| std::mem::discriminant(r) == first)
}
