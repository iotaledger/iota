// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! CommitSyncer implements efficient synchronization of committed data.
//!
//! During the operation of a committee of authorities for consensus, one or
//! more authorities can fall behind the quorum in their received and accepted
//! blocks. This can happen due to network disruptions, host crash, or other
//! reasons. Authorities fell behind need to catch up to the quorum to be able
//! to vote on the latest leaders. So efficient synchronization is necessary
//! to minimize the impact of temporary disruptions and maintain smooth
//! operations of the network.
//! CommitSyncer achieves efficient synchronization by relying on the following:
//! when blocks are included in commits with >= 2f+1 certifiers by stake, these
//! blocks must have passed verifications on some honest validators, so
//! re-verifying them is unnecessary. In fact, the quorum certified commits
//! themselves can be trusted to be sent to IOTA directly, but for simplicity
//! this is not done. Blocks from trusted commits still go through Core and
//! committer.
//!
//! Another way CommitSyncer improves the efficiency of synchronization is
//! parallel fetching: commits have a simple dependency graph (linear), so it is
//! easy to fetch ranges of commits in parallel.
//!
//! Commit synchronization is an expensive operation, involving transferring
//! large amount of data via the network. And it is not on the critical path of
//! block processing. So the heuristics for synchronization, including triggers
//! and retries, should be chosen to favor throughput and efficient resource
//! usage, over faster reactions.

pub mod fast;
pub mod regular;

use std::{collections::BTreeMap, sync::Arc};

use bytes::Bytes;
use parking_lot::RwLock;
use starfish_config::AuthorityIndex;
use tokio::{sync::oneshot, task::JoinHandle};

use crate::{
    BlockRef, CommitConsumerMonitor, Transaction, VerifiedBlockHeader,
    block_header::{
        BlockHeaderAPI, SignedBlockHeader, TransactionsCommitment, VerifiedTransactions,
    },
    block_verifier::BlockVerifier,
    commit::{Commit, CommitAPI as _, CommitDigest, CommitRange, CommitRef, TrustedCommit},
    commit_vote_monitor::CommitVoteMonitor,
    context::Context,
    core_thread::CoreThreadDispatcher,
    dag_state::DagState,
    encoder::create_encoder,
    error::{ConsensusError, ConsensusResult},
    network::NetworkClient,
    stake_aggregator::{QuorumThreshold, StakeAggregator},
    transaction_ref::GenericTransactionRef,
};

pub(crate) enum CommitSyncType {
    Fast,
    Regular,
}

impl CommitSyncType {
    pub(crate) fn commit_sync_batch_size(&self, context: &Context) -> u32 {
        match self {
            CommitSyncType::Fast => context.parameters.fast_commit_sync_batch_size,
            CommitSyncType::Regular => context.parameters.commit_sync_batch_size,
        }
    }

    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            CommitSyncType::Fast => "fast_commit_sync",
            CommitSyncType::Regular => "commit_sync",
        }
    }
}

// Handle to stop the CommitSyncer loop.
pub(crate) struct CommitSyncerHandle {
    schedule_task: JoinHandle<()>,
    tx_shutdown: oneshot::Sender<()>,
}

impl CommitSyncerHandle {
    pub(crate) async fn stop(self) {
        let _ = self.tx_shutdown.send(());
        // Do not abort schedule task, which waits for fetches to shut down.
        if let Err(e) = self.schedule_task.await {
            if e.is_panic() {
                std::panic::resume_unwind(e.into_panic());
            }
        }
    }
}

pub(crate) struct Inner<C: NetworkClient> {
    pub(crate) context: Arc<Context>,
    pub(crate) core_thread_dispatcher: Arc<dyn CoreThreadDispatcher>,
    pub(crate) commit_vote_monitor: Arc<CommitVoteMonitor>,
    pub(crate) commit_consumer_monitor: Arc<CommitConsumerMonitor>,
    pub(crate) network_client: Arc<C>,
    pub(crate) block_verifier: Arc<dyn BlockVerifier>,
    pub(crate) dag_state: Arc<RwLock<DagState>>,
    pub(crate) sync_type: CommitSyncType,
}

impl<C: NetworkClient> Inner<C> {
    /// Verifies the commits and also certifies them using the provided vote
    /// blocks for the last commit. The method returns the trusted commits
    /// and the votes as verified blocks.
    pub(crate) fn verify_commits(
        &self,
        peer: AuthorityIndex,
        commit_range: CommitRange,
        serialized_commits: Vec<Bytes>,
        serialized_vote_blocks_headers: Vec<Bytes>,
    ) -> ConsensusResult<Vec<TrustedCommit>> {
        // Parse and verify commits.
        let mut commits = Vec::new();
        for serialized in &serialized_commits {
            let commit: Commit =
                bcs::from_bytes(serialized).map_err(ConsensusError::MalformedCommit)?;
            let digest = TrustedCommit::compute_digest(serialized);
            if commits.is_empty() {
                // start is inclusive, so first commit must be at the start index.
                if commit.index() != commit_range.start() {
                    return Err(ConsensusError::UnexpectedStartCommit {
                        peer,
                        start: commit_range.start(),
                        commit: Box::new(commit),
                    });
                }
            } else {
                // Verify next commit increments index and references the previous digest.
                let (last_commit_digest, last_commit): &(CommitDigest, Commit) =
                    commits.last().unwrap();
                if commit.index() != last_commit.index() + 1
                    || &commit.previous_digest() != last_commit_digest
                {
                    return Err(ConsensusError::UnexpectedCommitSequence {
                        peer,
                        prev_commit: Box::new(last_commit.clone()),
                        curr_commit: Box::new(commit),
                    });
                }
            }
            // Do not process more commits past the end index.
            if commit.index() > commit_range.end() {
                break;
            }
            commits.push((digest, commit));
        }
        let Some((end_commit_digest, end_commit)) = commits.last() else {
            return Err(ConsensusError::NoCommitReceived { peer });
        };

        // Parse and verify blocks. Then accumulate votes on the end commit.
        let end_commit_ref = CommitRef::new(end_commit.index(), *end_commit_digest);
        let mut stake_aggregator = StakeAggregator::<QuorumThreshold>::new();
        for serialized_block_header in serialized_vote_blocks_headers.into_iter() {
            let signed_block_header: SignedBlockHeader = bcs::from_bytes(&serialized_block_header)
                .map_err(ConsensusError::MalformedHeader)?;
            // The block signature needs to be verified.
            self.block_verifier.verify(&signed_block_header)?;
            for vote in signed_block_header.commit_votes() {
                if *vote == end_commit_ref {
                    stake_aggregator.add(signed_block_header.author(), &self.context.committee);
                }
            }
        }

        // Check if the end commit has enough votes.
        if !stake_aggregator.reached_threshold(&self.context.committee) {
            return Err(ConsensusError::NotEnoughCommitVotes {
                stake: stake_aggregator.stake(),
                peer,
                commit: Box::new(end_commit.clone()),
            });
        }

        let trusted_commits = commits
            .into_iter()
            .zip(serialized_commits)
            .map(|((_d, c), s)| TrustedCommit::new_trusted(c, s))
            .collect();
        Ok(trusted_commits)
    }
}

/// Verifies transactions against their block headers and returns a map of
/// BlockRef to VerifiedTransactions.
pub(crate) fn verify_transactions_with_headers(
    context: Arc<Context>,
    peer: AuthorityIndex,
    serialized_transactions: BTreeMap<GenericTransactionRef, Bytes>,
    block_headers: BTreeMap<BlockRef, VerifiedBlockHeader>,
) -> ConsensusResult<BTreeMap<GenericTransactionRef, VerifiedTransactions>> {
    let mut verified_transactions_map = BTreeMap::new();
    let mut encoder = create_encoder(&context);
    for (committed_transactions_ref, inner_serialized_transactions) in serialized_transactions {
        let block_ref = match committed_transactions_ref {
            GenericTransactionRef::BlockRef(br) => br,
            _ => {
                return Err(ConsensusError::TransactionRefVariantMismatch {
                    protocol_flag_enabled: false,
                    expected_variant: "BlockRef",
                    received_variant: "TransactionRef",
                });
            }
        };
        // Step 1: Get the block header and verify that the transactions commitment
        // matches. This ensures the transactions we received are exactly
        // the ones that were included in the block when it was created.
        let block_header = block_headers
            .get(&block_ref)
            .expect("header for fetched transactions must exist");

        if block_header.transactions_commitment()
            != TransactionsCommitment::compute_transactions_commitment(
                &inner_serialized_transactions,
                &context,
                &mut encoder,
            )
            .expect("correct computation of the transactions commitment should be successful")
        {
            return Err(ConsensusError::TransactionCommitmentFailure {
                round: block_ref.round,
                author: block_ref.author,
                peer,
            });
        }

        // Step 2: Deserialize the actual transactions vector.
        let transactions: Vec<Transaction> = bcs::from_bytes(&inner_serialized_transactions)
            .map_err(ConsensusError::MalformedTransactions)?;

        // Step 3: Create a VerifiedTransactions instance and insert into map
        let verified_transactions = VerifiedTransactions::new(
            transactions,
            block_ref,
            block_header.transactions_commitment(),
            inner_serialized_transactions,
        );

        verified_transactions_map.insert(
            GenericTransactionRef::BlockRef(block_ref),
            verified_transactions,
        );
    }

    Ok(verified_transactions_map)
}

/// Verifies transactions against their transaction refs and returns a map of
/// BlockRef to VerifiedTransactions.
pub(crate) fn verify_transactions_with_transactions_refs(
    context: &Arc<Context>,
    peer: AuthorityIndex,
    serialized_transactions: BTreeMap<GenericTransactionRef, Bytes>,
) -> ConsensusResult<BTreeMap<GenericTransactionRef, VerifiedTransactions>> {
    let mut verified_transactions_map = BTreeMap::new();
    let mut encoder = create_encoder(context);
    for (committed_transactions_ref, inner_serialized_transactions) in serialized_transactions {
        let transaction_ref = match committed_transactions_ref {
            GenericTransactionRef::TransactionRef(tx_ref) => tx_ref,
            _ => {
                return Err(ConsensusError::TransactionRefVariantMismatch {
                    protocol_flag_enabled: true,
                    expected_variant: "TransactionRef",
                    received_variant: "BlockRef",
                });
            }
        };
        let block_ref = BlockRef {
            round: transaction_ref.round,
            author: transaction_ref.author,
            digest: transaction_ref.block_digest,
        };
        // Step 1: Verify that the transaction commitment matches.
        if transaction_ref.transactions_commitment
            != TransactionsCommitment::compute_transactions_commitment(
                &inner_serialized_transactions,
                context,
                &mut encoder,
            )
            .expect("correct computation of the transactions commitment should be successful")
        {
            return Err(ConsensusError::TransactionCommitmentFailure {
                round: transaction_ref.round,
                author: transaction_ref.author,
                peer,
            });
        }

        // Step 2: Deserialize the actual transactions vector.
        let transactions: Vec<Transaction> = bcs::from_bytes(&inner_serialized_transactions)
            .map_err(ConsensusError::MalformedTransactions)?;

        // Step 3: Create a VerifiedTransactions instance and insert into map
        let verified_transactions = VerifiedTransactions::new(
            transactions,
            block_ref,
            transaction_ref.transactions_commitment,
            inner_serialized_transactions,
        );

        verified_transactions_map.insert(
            GenericTransactionRef::TransactionRef(transaction_ref),
            verified_transactions,
        );
    }

    Ok(verified_transactions_map)
}
