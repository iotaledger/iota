// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{ops::Bound::Included, time::Duration};

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
        BlockHeaderAPI as _, BlockHeaderDigest, BlockRef, Round, TransactionsCommitment,
        VerifiedBlock, VerifiedBlockHeader, VerifiedTransactions,
    },
    commit::{CommitAPI as _, CommitDigest, CommitIndex, CommitRange, CommitRef, TrustedCommit},
    error::{ConsensusError, ConsensusResult},
    network::SerializedHeaderAndTransactions,
};

/// Persistent storage with RocksDB.
pub(crate) struct RocksDBStore {
    /// Stores SignedBlockHeader by refs.
    block_headers: DBMap<(Round, AuthorityIndex, BlockHeaderDigest), Bytes>,
    /// Stores SignedBlock by refs.
    transactions: DBMap<(Round, AuthorityIndex, BlockHeaderDigest), Bytes>,
    /// A secondary index that orders refs first by authors.
    digests_by_authorities: DBMap<(AuthorityIndex, Round, BlockHeaderDigest), ()>,
    /// Maps commit index to Commit.
    commits: DBMap<(CommitIndex, CommitDigest), Bytes>,
    /// Collects votes on commits.
    /// TODO: batch multiple votes into a single row.
    commit_votes: DBMap<(CommitIndex, CommitDigest, BlockRef), ()>,
    /// Stores info related to Commit that helps recovery.
    commit_info: DBMap<(CommitIndex, CommitDigest), CommitInfo>,
}

impl RocksDBStore {
    const TRANSACTIONS_CF: &'static str = "transactions";
    const BLOCK_HEADERS_CF: &'static str = "block_headers";
    const DIGESTS_BY_AUTHORITIES_CF: &'static str = "digests";
    const COMMITS_CF: &'static str = "commits";
    const COMMIT_VOTES_CF: &'static str = "commit_votes";
    const COMMIT_INFO_CF: &'static str = "commit_info";

    /// Creates a new instance of RocksDB storage.
    pub(crate) fn new(path: &str) -> Self {
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
                Self::BLOCK_HEADERS_CF,
                default_db_options()
                    .optimize_for_write_throughput_no_deletion()
                    // TODO:think about these constants, for now it is a copy from blocks
                    .set_block_options(512, 128 << 10)
                    .options,
            ),
            (Self::DIGESTS_BY_AUTHORITIES_CF, cf_options.clone()),
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
            digests_by_authorities,
            commits,
            commit_votes,
            commit_info,
        ) = reopen!(&rocksdb,
            Self::BLOCK_HEADERS_CF;<(Round, AuthorityIndex, BlockHeaderDigest), Bytes>,
            Self::TRANSACTIONS_CF;<(Round, AuthorityIndex, BlockHeaderDigest), Bytes>,
            Self::DIGESTS_BY_AUTHORITIES_CF;<(AuthorityIndex, Round, BlockHeaderDigest), ()>,
            Self::COMMITS_CF;<(CommitIndex, CommitDigest), Bytes>,
            Self::COMMIT_VOTES_CF;<(CommitIndex, CommitDigest, BlockRef), ()>,
            Self::COMMIT_INFO_CF;<(CommitIndex, CommitDigest), CommitInfo>
        );

        Self {
            block_headers,
            transactions,
            digests_by_authorities,
            commits,
            commit_votes,
            commit_info,
        }
    }
}

impl Store for RocksDBStore {
    fn write(&self, write_batch: WriteBatch) -> ConsensusResult<()> {
        fail_point!("consensus-store-before-write");
        // TODO: does it matter which CF we use here?
        let mut batch = self.transactions.batch();

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
            let block_ref = transaction.block_ref();
            batch
                .insert_batch(
                    &self.transactions,
                    [(
                        (block_ref.round, block_ref.author, block_ref.digest),
                        transaction.serialized(),
                    )],
                )
                .map_err(ConsensusError::RocksDBFailure)?;
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
        let keys = refs
            .iter()
            .map(|r| (r.round, r.author, r.digest))
            .collect::<Vec<_>>();

        // TODO: is consistency guaranteed here? what if there is a write between those
        //  two reads?
        let serialized_vec_transactions = self.transactions.multi_get(keys.clone())?;
        let serialized_block_headers = self.block_headers.multi_get(keys)?;
        let mut blocks = vec![];
        for ((key, serialized_block_header), serialized_transactions) in refs
            .iter()
            .zip(serialized_block_headers)
            .zip(serialized_vec_transactions)
        {
            if let (Some(serialized_block_header), Some(serialized_transactions)) =
                (serialized_block_header, serialized_transactions)
            {
                let block = VerifiedBlock::try_from(SerializedHeaderAndTransactions {
                    serialized_block_header,
                    serialized_transactions,
                })?;

                // Makes sure block data is not corrupted by comparing digests.
                assert_eq!(*key, block.reference());
                blocks.push(Some(block));
            } else {
                blocks.push(None);
            }
        }
        Ok(blocks)
    }

    fn read_block_headers(
        &self,
        refs: &[BlockRef],
    ) -> ConsensusResult<Vec<Option<VerifiedBlockHeader>>> {
        let keys = refs
            .iter()
            .map(|r| (r.round, r.author, r.digest))
            .collect::<Vec<_>>();
        let serialized_block_headers = self.block_headers.multi_get(keys)?;
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

    fn read_transactions(
        &self,
        refs: &[BlockRef],
    ) -> ConsensusResult<Vec<Option<VerifiedTransactions>>> {
        let keys = refs
            .iter()
            .map(|r| (r.round, r.author, r.digest))
            .collect::<Vec<_>>();
        let serialized_transactions = self.transactions.multi_get(keys)?;
        let mut result = Vec::with_capacity(refs.len());
        for (i, serialized) in serialized_transactions.into_iter().enumerate() {
            if let Some(bytes) = serialized {
                let transactions: Vec<Transaction> =
                    bcs::from_bytes(&bytes).map_err(ConsensusError::MalformedTransactions)?;
                let commitment = TransactionsCommitment::compute_transactions_commitment(&bytes)
                    .expect("computation of the transactions commitment should not fail");
                let verified = VerifiedTransactions::new(transactions, refs[i], commitment, bytes);
                result.push(Some(verified));
            } else {
                result.push(None);
            }
        }
        Ok(result)
    }

    fn contains_transactions(&self, refs: &[BlockRef]) -> ConsensusResult<Vec<bool>> {
        let keys = refs
            .iter()
            .map(|r| (r.round, r.author, r.digest))
            .collect::<Vec<_>>();
        let exist = self.transactions.multi_contains_keys(keys)?;
        Ok(exist)
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

    fn scan_block_headers_by_author(
        &self,
        author: AuthorityIndex,
        start_round: Round,
    ) -> ConsensusResult<Vec<VerifiedBlockHeader>> {
        let mut refs = vec![];
        for kv in self.digests_by_authorities.safe_range_iter((
            Included((author, start_round, BlockHeaderDigest::MIN)),
            Included((author, Round::MAX, BlockHeaderDigest::MAX)),
        )) {
            let ((author, round, digest), _) = kv?;
            refs.push(BlockRef::new(round, author, digest));
        }
        let results = self.read_block_headers(refs.as_slice())?;
        let mut block_headers = Vec::with_capacity(refs.len());
        for (r, block) in refs.into_iter().zip(results.into_iter()) {
            block_headers.push(
                block.unwrap_or_else(|| panic!("Storage inconsistency: block {:?} not found!", r)),
            );
        }
        Ok(block_headers)
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
                block.unwrap_or_else(|| panic!("Storage inconsistency: block {:?} not found!", r)),
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
            .safe_range_iter((
                Included((author, Round::MIN, BlockHeaderDigest::MIN)),
                Included((author, before_round, BlockHeaderDigest::MAX)),
            ))
            .skip_to_last()
            .reverse()
            .take(num_of_rounds as usize)
        {
            let ((author, round, digest), _) = kv?;
            refs.push_front(BlockRef::new(round, author, digest));
        }
        let results = self.read_blocks(refs.as_slices().0)?;
        let mut blocks = vec![];
        for (r, block) in refs.into_iter().zip(results.into_iter()) {
            blocks.push(
                block.unwrap_or_else(|| panic!("Storage inconsistency: block {:?} not found!", r)),
            );
        }
        Ok(blocks)
    }

    fn read_last_commit(&self) -> ConsensusResult<Option<TrustedCommit>> {
        let Some(result) = self.commits.safe_iter().skip_to_last().next() else {
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
        let Some(result) = self.commit_info.safe_iter().skip_to_last().next() else {
            return Ok(None);
        };
        let (key, commit_info) = result.map_err(ConsensusError::RocksDBFailure)?;
        Ok(Some((CommitRef::new(key.0, key.1), commit_info)))
    }
}
