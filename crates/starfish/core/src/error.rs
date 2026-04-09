// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use fastcrypto::error::FastCryptoError;
use starfish_config::{AuthorityIndex, Committee, Epoch, Stake};
use strum_macros::IntoStaticStr;
use thiserror::Error;
use typed_store::TypedStoreError;

use crate::{
    block_header::{BlockRef, GENESIS_ROUND, Round},
    commit::{Commit, CommitIndex},
    transaction_ref::{GenericTransactionRef, GenericTransactionRefAPI as _, TransactionRef},
};

/// Errors that can occur when processing blocks, reading from storage, or
/// encountering shutdown.
#[derive(Clone, Debug, Error, IntoStaticStr)]
pub(crate) enum ConsensusError {
    #[error("Error deserializing block header: {0}")]
    MalformedHeader(bcs::Error),

    #[error("Error deserializing shard with proof: {0}")]
    MalformedShard(bcs::Error),

    #[error("Error deserializing block transactions: {0}")]
    MalformedTransactions(bcs::Error),

    #[error("Error deserializing commit: {0}")]
    MalformedCommit(bcs::Error),

    #[error("Error serializing: {0}")]
    SerializationFailure(bcs::Error),

    #[error("Error deserializing: {0}")]
    DeserializationFailure(bcs::Error),

    #[error("Block contains a transaction that is too large: {size} > {limit}")]
    TransactionTooLarge { size: usize, limit: usize },

    #[error("Block contains too many transactions: {count} > {limit}")]
    TooManyTransactions { count: usize, limit: usize },

    #[error("Block contains too many transaction bytes: {size} > {limit}")]
    TooManyTransactionBytes { size: usize, limit: usize },

    #[error("Unexpected block authority {0} from peer {1}")]
    UnexpectedAuthority(AuthorityIndex, AuthorityIndex),

    #[error("Block has wrong epoch: expected {expected}, actual {actual}")]
    WrongEpoch { expected: Epoch, actual: Epoch },

    #[error("Genesis block headers should only be generated from Committee!")]
    UnexpectedGenesisHeader,

    #[error("Genesis block headers or transactions are requested from {peer}!")]
    UnexpectedGenesisRequested { peer: AuthorityIndex },

    #[error(
        "Expected {requested} but received {received_headers} block headers from authority {authority}"
    )]
    UnexpectedNumberOfHeadersFetched {
        authority: AuthorityIndex,
        requested: usize,
        received_headers: usize,
    },

    #[error(
        "Unexpected block header {block_ref} returned while fetching last own header from peer {index}"
    )]
    UnexpectedLastOwnHeader {
        index: AuthorityIndex,
        block_ref: BlockRef,
    },

    #[error(
        "Too many transactions have been returned from authority {0} when requesting to fetch missing transactions"
    )]
    TooManyFetchedTransactionsReturned(AuthorityIndex),

    #[error("Too many authorities have been provided from authority {0}")]
    TooManyAuthoritiesProvided(AuthorityIndex),

    #[error(
        "Provided size of highest accepted rounds parameter, {0}, is different than committee size, {1}"
    )]
    InvalidSizeOfHighestAcceptedRounds(usize, usize),

    #[error("Invalid authority index: {index} > {max}")]
    InvalidAuthorityIndex { index: AuthorityIndex, max: usize },

    #[error("Invalid authority index: {index} > {max} from peer {peer}")]
    InvalidAuthorityIndexRequested {
        index: AuthorityIndex,
        max: usize,
        peer: AuthorityIndex,
    },

    #[error("Failed to deserialize signature: {0}")]
    MalformedSignature(FastCryptoError),

    #[error("Failed to verify the block's signature: {0}")]
    SignatureVerificationFailure(FastCryptoError),

    #[error("Wrong transaction commitment in B{round} by {author} received from {peer}")]
    TransactionCommitmentFailure {
        round: Round,
        author: AuthorityIndex,
        peer: AuthorityIndex,
    },
    #[error(
        "After reconstruction, the transaction commitment does not match the commitment in transaction ref {}",
        transaction_ref
    )]
    TransactionCommitmentMismatch { transaction_ref: TransactionRef },

    #[error("Synchronizer for fetching blocks directly from {0} is saturated")]
    SynchronizerSaturated(AuthorityIndex),

    #[error("Transaction Synchronizer is saturated")]
    TransactionSynchronizerSaturated,

    #[error("Block {block_ref:?} rejected: {reason}")]
    BlockRejected { block_ref: BlockRef, reason: String },

    #[error(
        "Ancestor is in wrong position: block {block_authority}, ancestor {ancestor_authority}, position {position}"
    )]
    InvalidAncestorPosition {
        block_authority: AuthorityIndex,
        ancestor_authority: AuthorityIndex,
        position: usize,
    },

    #[error("Ancestor's round ({ancestor}) should be lower than the block's round ({block})")]
    InvalidAncestorRound { ancestor: Round, block: Round },

    #[error("Ancestor {0} not found among genesis blocks!")]
    InvalidGenesisAncestor(BlockRef),

    #[error("Too many ancestors in the block: {0} > {1}")]
    TooManyAncestors(usize, usize),

    #[error(
        "Commit range exceeded limit after scanning during {sync_type} sync: {count} > {limit}"
    )]
    CommitRangeExceededAfterScanning {
        count: CommitIndex,
        limit: CommitIndex,
        sync_type: &'static str,
    },

    #[error("Peer {peer} sent too many commits: {count} > {limit}")]
    TooManyCommitsFromPeer {
        peer: AuthorityIndex,
        count: CommitIndex,
        limit: CommitIndex,
    },

    #[error("Ancestors from the same authority {0}")]
    DuplicatedAncestorsAuthority(AuthorityIndex),

    #[error("Insufficient stake from parents: {parent_stakes} < {quorum}")]
    InsufficientParentStakes { parent_stakes: Stake, quorum: Stake },

    #[error("Invalid transaction: {0}")]
    InvalidTransaction(String),

    #[error("Received no commit from peer {peer}")]
    NoCommitReceived { peer: AuthorityIndex },

    #[error(
        "Received unexpected start commit from peer {peer}: requested {start}, received {commit:?}"
    )]
    UnexpectedStartCommit {
        peer: AuthorityIndex,
        start: CommitIndex,
        commit: Box<Commit>,
    },

    #[error(
        "Received unexpected commit sequence from peer {peer}: {prev_commit:?}, {curr_commit:?}"
    )]
    UnexpectedCommitSequence {
        peer: AuthorityIndex,
        prev_commit: Box<Commit>,
        curr_commit: Box<Commit>,
    },

    #[error("Not enough votes ({stake}) on end commit from peer {peer}: {commit:?}")]
    NotEnoughCommitVotes {
        stake: Stake,
        peer: AuthorityIndex,
        commit: Box<Commit>,
    },

    #[error("Received unexpected block header from peer {peer}: {requested:?} vs {received:?}")]
    UnexpectedBlockHeaderForCommit {
        peer: AuthorityIndex,
        requested: BlockRef,
        received: BlockRef,
    },

    #[error("Received unexpected transaction from peer {peer}: {received:?}")]
    UnexpectedTransactionForCommit {
        peer: AuthorityIndex,
        received: GenericTransactionRef,
    },

    #[error(
        "Fetched transactions from peer {peer} do not match committed transaction refs. Expected {expected} transactions, but received {received} transactions"
    )]
    FetchedTransactionsMismatch {
        peer: AuthorityIndex,
        expected: usize,
        received: usize,
    },

    #[error("RocksDB failure: {0}")]
    RocksDBFailure(#[from] TypedStoreError),

    #[error("Network config error: {0:?}")]
    NetworkConfig(String),

    #[error("Failed to connect as client: {0:?}")]
    NetworkClientConnection(String),

    #[error("Failed to send request: {0:?}")]
    NetworkRequest(String),

    #[error("Request timeout: {0:?}")]
    NetworkRequestTimeout(String),

    #[error("Accumulator sender has shut down!")]
    AccumulatorSenderClosed,

    #[error("Consensus has shut down!")]
    Shutdown,

    #[error("Shard encoder reset failed: {0}")]
    EncoderResetFailed(String),

    #[error("Failed to add original shard to encoder: {0}")]
    AddShardFailed(String),

    #[error("Reed-Solomon encoding failed in encoder: {0}")]
    ShardsEncodingFailed(String),

    #[error("Reed-Solomon decoding failed in decoder: {0}")]
    ShardsDecodingFailed(String),

    #[error(
        "Shards collection does not contain enough valid shards for decoding: {0} found, at least {1} needed"
    )]
    InsufficientShardsInDecoder(usize, usize),

    #[error("Vector of shards is too small: {0} bytes found, at least {1} bytes needed")]
    ShardsVecIsTooSmall(usize, usize),

    #[error(
        "Round of the header in a bundle is greater or equal to the block round: {header_round} >= {block_round}"
    )]
    TooBigHeaderRoundInABundle {
        header_round: Round,
        block_round: Round,
    },

    #[error("Block bundle from {peer} contains shard from round {round} with incorrect proof")]
    IncorrectShardProof { peer: AuthorityIndex, round: Round },

    #[error(
        "Round of the shard in a bundle is greater or equal to the block round: {shard_round} >= {block_round}"
    )]
    TooBigShardRoundInABundle {
        shard_round: Round,
        block_round: Round,
    },

    #[error(
        "All GenericTransactionRef elements must have the same variant (BlockRef, TransactionRef, etc.) for batch operations."
    )]
    InconsistentTransactionRefVariants,

    #[error(
        "Transaction reference variant is inconsistent with protocol flag consensus_fast_commit_sync={protocol_flag_enabled}. Expected {expected_variant}, but received {received_variant}"
    )]
    TransactionRefVariantMismatch {
        protocol_flag_enabled: bool,
        expected_variant: &'static str,
        received_variant: &'static str,
    },

    #[error("Failed to fetch {num_requested} block headers from any peer")]
    FailedToFetchBlockHeaders { num_requested: usize },

    #[error("Voting block header {block_ref:?} for commit certification was not found in storage")]
    MissingVotingBlockHeaderInStorage { block_ref: BlockRef },

    // TODO: This error can be removed once consensus_fast_commit_sync is enabled on all networks.
    // It's currently used to gate fast commit sync endpoints and features during the gradual
    // rollout phase.
    #[error("Fast commit sync is not enabled in the current protocol version")]
    FastCommitSyncNotEnabled,
}

impl ConsensusError {
    /// Returns the error name - only the enum name without any parameters - as
    /// a static string.
    pub fn name(&self) -> &'static str {
        self.into()
    }

    pub fn quick_validation_requested_block_refs(
        block_refs: &[BlockRef],
        peer: AuthorityIndex,
        committee: &Committee,
    ) -> ConsensusResult<()> {
        for block in block_refs {
            if !committee.is_valid_index(block.author) {
                return Err(ConsensusError::InvalidAuthorityIndexRequested {
                    index: block.author,
                    max: committee.size(),
                    peer,
                });
            }
            if block.round == GENESIS_ROUND {
                return Err(ConsensusError::UnexpectedGenesisRequested { peer });
            }
        }
        Ok(())
    }

    pub fn quick_validation_requested_tx_refs(
        gen_tx_refs: &[GenericTransactionRef],
        peer: AuthorityIndex,
        committee: &Committee,
    ) -> ConsensusResult<()> {
        for gen_tx_ref in gen_tx_refs {
            if !committee.is_valid_index(gen_tx_ref.author()) {
                return Err(ConsensusError::InvalidAuthorityIndexRequested {
                    index: gen_tx_ref.author(),
                    max: committee.size(),
                    peer,
                });
            }
            if gen_tx_ref.round() == GENESIS_ROUND {
                return Err(ConsensusError::UnexpectedGenesisRequested { peer });
            }
        }
        Ok(())
    }

    pub fn quick_validation_authority_indices(
        authorities: &[AuthorityIndex],
        committee: &Committee,
    ) -> ConsensusResult<()> {
        // Ensure that those are valid authorities
        for authority in authorities {
            if !committee.is_valid_index(*authority) {
                return Err(ConsensusError::InvalidAuthorityIndex {
                    index: *authority,
                    max: committee.size(),
                });
            }
        }
        Ok(())
    }
}

pub type ConsensusResult<T> = Result<T, ConsensusError>;

#[macro_export]
macro_rules! bail {
    ($e:expr) => {
        return Err($e);
    };
}

#[macro_export(local_inner_macros)]
macro_rules! ensure {
    ($cond:expr, $e:expr) => {
        if !($cond) {
            bail!($e);
        }
    };
}

#[cfg(test)]
mod test {
    use super::*;
    /// This test ensures that consensus errors when converted to a static
    /// string are the same as the enum name without any parameterers
    /// included to the result string.
    #[test]
    fn test_error_name() {
        {
            let error = ConsensusError::InvalidAncestorRound {
                ancestor: 10,
                block: 11,
            };
            let error: &'static str = error.into();
            assert_eq!(error, "InvalidAncestorRound");
        }
        {
            let error = ConsensusError::InvalidAuthorityIndex {
                index: AuthorityIndex::new_for_test(3),
                max: 10,
            };
            assert_eq!(error.name(), "InvalidAuthorityIndex");
        }
        {
            let error = ConsensusError::InsufficientParentStakes {
                parent_stakes: 5,
                quorum: 20,
            };
            assert_eq!(error.name(), "InsufficientParentStakes");
        }
    }
}
