// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This module defines the network interface, and provides network
//! implementations for the consensus protocol.
//!
//! Having an abstract network interface allows
//! - simplifying the semantics of sending data and serving requests over the
//!   network
//! - hiding implementation specific types and semantics from the consensus
//!   protocol
//! - allowing easy swapping of network implementations, for better performance
//!   or testing
//!
//! When modifying the client and server interfaces, the principle is to keep
//! the interfaces low level, close to underlying implementations in semantics.
//! For example, the client interface exposes sending messages to a specific
//! peer, instead of broadcasting to all peers. Subscribing to a stream of
//! blocks gets back the stream via response, instead of delivering the stream
//! directly to the server. This keeps the logic agnostics to the underlying
//! network outside of this module, so they can be reused easily across network
//! implementations.

use std::{collections::BTreeSet, pin::Pin, time::Duration};

use async_trait::async_trait;
use bytes::Bytes;
use futures::Stream;
use serde::{Deserialize, Serialize};
use starfish_config::AuthorityIndex;

use crate::{
    Round, VerifiedBlockHeader,
    block_header::{BlockRef, VerifiedBlock},
    commit::{CommitRange, TrustedCommit},
    error::{ConsensusError, ConsensusResult},
};

// Tonic generated RPC stubs.
mod tonic_gen {
    include!(concat!(env!("OUT_DIR"), "/consensus.ConsensusService.rs"));
}

pub(crate) mod metrics;
mod metrics_layer;
#[cfg(all(test, not(msim)))]
mod network_tests;
#[cfg(test)]
pub(crate) mod test_network;
#[cfg(not(msim))]
pub(crate) mod tonic_network;
#[cfg(msim)]
pub mod tonic_network;
mod tonic_tls;

use crate::{
    commit_syncer::CommitSyncType,
    encoder::ShardEncoder,
    transaction_ref::{GenericTransactionRef, TransactionRef},
};

/// Controls transaction fetching truncation behavior for different sync modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TransactionFetchMode {
    /// No truncation - used by fast commit sync which fetches all transactions
    /// referenced by commits in a batch
    FastCommitSync,
    /// Truncate to the maximum of max_transactions_per_commit_sync_fetch and
    /// max_transactions_per_regular_sync_fetch- used by regular commit sync
    /// and transactions synchronizer
    TransactionSync,
}

/// A stream of serialized blocks with additional information such as headers or
/// shards.
pub(crate) type BlockBundleStream = Pin<Box<dyn Stream<Item = SerializedBlockBundle> + Send>>;

/// Network client for communicating with peers.
///
/// NOTE: the timeout parameters help saving resources at client and potentially
/// server. But it is up to the server implementation if the timeout is honored.
/// - To bound server resources, server should implement own timeout for
///   incoming requests.
#[async_trait]
pub(crate) trait NetworkClient: Send + Sync + Sized + 'static {
    /// Subscribes to blocks from a peer after last_received round.
    #[allow(dead_code)]
    async fn subscribe_block_bundles(
        &self,
        peer: AuthorityIndex,
        last_received: Round,
        timeout: Duration,
    ) -> ConsensusResult<BlockBundleStream>;

    /// Fetches transactions for the given block references from a peer.
    async fn fetch_transactions(
        &self,
        peer: AuthorityIndex,
        transactions_refs: Vec<GenericTransactionRef>,
        timeout: Duration,
    ) -> ConsensusResult<Vec<Bytes>>;

    // TODO: add a parameter for maximum total size of blocks returned.
    /// Fetches serialized `SignedBlockHeader`s from a peer. It also might
    /// return additional ancestor blocks of the requested blocks according
    /// to the provided `highest_accepted_rounds`. The
    /// `highest_accepted_rounds` length should be equal to the committee
    /// size. If `highest_accepted_rounds` is empty then it will be simply
    /// ignored.
    async fn fetch_block_headers(
        &self,
        peer: AuthorityIndex,
        block_refs: Vec<BlockRef>,
        highest_accepted_rounds: Vec<Round>,
        timeout: Duration,
    ) -> ConsensusResult<Vec<Bytes>>;

    /// Fetches serialized commits in the commit range from a peer.
    /// Returns a tuple of both the serialized commits and serialized headers
    /// that contain votes certifying the last commit.
    async fn fetch_commits(
        &self,
        peer: AuthorityIndex,
        commit_range: CommitRange,
        timeout: Duration,
    ) -> ConsensusResult<(Vec<Bytes>, Vec<Bytes>)>;

    /// Fetches serialized commits in the commit range from a peer, headers
    /// voting for the last commit, and all transactions from these commits.
    /// Returns serialized commits, serialized headers voting for the last
    /// commit, and serialized transactions (as SerializedTransactionsV2 which
    /// includes TransactionRef). Used in the fast commit syncer.
    async fn fetch_commits_and_transactions(
        &self,
        peer: AuthorityIndex,
        commit_range: CommitRange,
        timeout: Duration,
    ) -> ConsensusResult<(Vec<Bytes>, Vec<Bytes>, Vec<Bytes>)>;

    /// Fetches the latest block from `peer` for the requested `authorities`.
    /// The latest blocks are returned in the serialised format of
    /// `SignedBlocks`. The method can return multiple blocks per peer as
    /// its possible to have equivocations.
    async fn fetch_latest_block_headers(
        &self,
        peer: AuthorityIndex,
        authorities: Vec<AuthorityIndex>,
        timeout: Duration,
    ) -> ConsensusResult<Vec<Bytes>>;
}

/// Network service for handling requests from peers.
#[async_trait]
pub(crate) trait NetworkService: Send + Sync + 'static {
    /// Handles the block and headers sent from the peer via subscription
    /// stream. Peer value can be trusted to be a valid authority index. But
    /// serialized_block must be verified before its contents are trusted.
    async fn handle_subscribed_block_bundle(
        &self,
        peer: AuthorityIndex,
        serialized_block_bundle: SerializedBlockBundle,
        encoder: &mut Box<dyn ShardEncoder + Send + Sync>,
    ) -> ConsensusResult<()>;

    /// Handles the subscription request from the peer.
    /// A stream of newly proposed blocks with additional data (headers or
    /// shards) is returned to the peer. The stream continues until the end
    /// of epoch, peer unsubscribes, or a network error / crash occurs.
    async fn handle_subscribe_block_bundles_request(
        &self,
        peer: AuthorityIndex,
        last_received: Round,
    ) -> ConsensusResult<BlockBundleStream>;

    /// Handles the request to fetch block headers by references from the peer.
    async fn handle_fetch_headers(
        &self,
        peer: AuthorityIndex,
        block_refs: Vec<BlockRef>,
        highest_accepted_rounds: Vec<Round>,
    ) -> ConsensusResult<Vec<Bytes>>;

    /// Handles the request to fetch commits by index range from the peer.
    /// Batch size limit depends on the sync type.
    async fn handle_fetch_commits(
        &self,
        peer: AuthorityIndex,
        commit_range: CommitRange,
        commit_sync_type: CommitSyncType,
    ) -> ConsensusResult<(Vec<TrustedCommit>, Vec<VerifiedBlockHeader>)>;

    /// Handles the request to fetch commits and transactions by index range
    /// from the peer. Used in fast commit sync.
    /// Returns (commits, certifier_block_headers, transactions) as serialized
    /// bytes. Each transaction is serialized as SerializedTransactionsV2
    /// which includes the TransactionRef.
    async fn handle_fetch_commits_and_transactions(
        &self,
        peer: AuthorityIndex,
        commit_range: CommitRange,
    ) -> ConsensusResult<(Vec<Bytes>, Vec<Bytes>, Vec<Bytes>)>;

    /// Handles the request to fetch the latest block headers for the provided
    /// `authorities`.
    async fn handle_fetch_latest_block_headers(
        &self,
        peer: AuthorityIndex,
        authorities: Vec<AuthorityIndex>,
    ) -> ConsensusResult<Vec<Bytes>>;

    /// Handles the request to fetch transactions by references from the peer.
    /// The `fetch_mode` parameter controls whether results should be truncated
    /// to respect maximum transaction limits.
    async fn handle_fetch_transactions(
        &self,
        peer: AuthorityIndex,
        block_refs: Vec<GenericTransactionRef>,
        fetch_mode: TransactionFetchMode,
    ) -> ConsensusResult<Vec<Bytes>>;
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SerializedBlock {
    pub(crate) serialized_block: Bytes,
}

impl TryFrom<SerializedHeaderAndTransactions> for SerializedBlock {
    type Error = ConsensusError;

    fn try_from(
        serialized_header_and_transactions: SerializedHeaderAndTransactions,
    ) -> ConsensusResult<Self> {
        let bytes = bcs::to_bytes(&serialized_header_and_transactions)
            .map_err(ConsensusError::SerializationFailure)?;
        Ok(Self {
            serialized_block: Bytes::from(bytes),
        })
    }
}

impl TryFrom<VerifiedBlock> for SerializedBlock {
    type Error = ConsensusError;
    fn try_from(verified_block: VerifiedBlock) -> ConsensusResult<Self> {
        let (serialized_block_header, serialized_transactions) = verified_block.serialized();
        let serialized_header_and_transactions = SerializedHeaderAndTransactions {
            serialized_block_header: serialized_block_header.clone(),
            serialized_transactions: serialized_transactions.clone(),
        };
        let bytes = bcs::to_bytes(&serialized_header_and_transactions)
            .map_err(ConsensusError::SerializationFailure)?;
        Ok(Self {
            serialized_block: Bytes::from(bytes),
        })
    }
}

#[derive(Clone, PartialEq, Eq, Default, Serialize, Deserialize, Debug)]
pub(crate) struct SerializedHeaderAndTransactions {
    pub(crate) serialized_block_header: Bytes,
    pub(crate) serialized_transactions: Bytes,
}

impl From<VerifiedBlock> for SerializedHeaderAndTransactions {
    fn from(verified_block: VerifiedBlock) -> Self {
        let (serialized_block_header, serialized_transactions) = verified_block.serialized();
        Self {
            serialized_block_header: serialized_block_header.clone(),
            serialized_transactions: serialized_transactions.clone(),
        }
    }
}

impl TryFrom<SerializedBlock> for SerializedHeaderAndTransactions {
    type Error = ConsensusError;

    fn try_from(serialized_block: SerializedBlock) -> ConsensusResult<Self> {
        bcs::from_bytes(&serialized_block.serialized_block).map_err(ConsensusError::MalformedHeader)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct BlockBundle {
    pub(crate) verified_block: VerifiedBlock,
    pub(crate) verified_headers: Vec<VerifiedBlockHeader>,
    pub(crate) serialized_shards: Vec<Bytes>,
    pub(crate) useful_headers_authors: BTreeSet<AuthorityIndex>,
    pub(crate) useful_shards_authors: BTreeSet<AuthorityIndex>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct SerializedBlockBundleParts {
    pub(crate) serialized_block: Bytes,
    pub(crate) serialized_headers: Vec<Bytes>,
    pub(crate) serialized_shards: Vec<Bytes>,
    pub(crate) useful_headers_authors_bitmask: [u64; 4],
    pub(crate) useful_shards_authors_bitmask: [u64; 4],
}

fn authority_set_to_bitmask(authorities: &BTreeSet<AuthorityIndex>) -> [u64; 4] {
    let mut bitmask = [0u64; 4];
    for authority_index in authorities {
        let index = authority_index.value();
        let array_index = index / 64;
        let bit_pos = index % 64;
        bitmask[array_index] |= 1u64 << bit_pos;
    }
    bitmask
}

fn bitmask_to_authority_set(bitmask: [u64; 4]) -> BTreeSet<AuthorityIndex> {
    let mut set = BTreeSet::new();
    for (array_index, &bits) in bitmask.iter().enumerate() {
        let mut bits = bits;
        let base = array_index * 64;
        while bits != 0 {
            let bit = bits.trailing_zeros() as usize;
            set.insert(AuthorityIndex::from((base + bit) as u8));
            bits &= bits - 1;
        }
    }
    set
}

impl SerializedBlockBundleParts {
    pub(crate) fn useful_headers_authors(&self) -> BTreeSet<AuthorityIndex> {
        bitmask_to_authority_set(self.useful_headers_authors_bitmask)
    }
    pub(crate) fn useful_shards_authors(&self) -> BTreeSet<AuthorityIndex> {
        bitmask_to_authority_set(self.useful_shards_authors_bitmask)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SerializedBlockBundle {
    pub(crate) serialized_block_bundle: Bytes,
}

impl TryFrom<VerifiedBlock> for SerializedBlockBundleParts {
    type Error = ConsensusError;
    fn try_from(verified_block: VerifiedBlock) -> ConsensusResult<Self> {
        let (serialized_block_header, serialized_transactions) = verified_block.serialized();
        let serialized_header_and_transactions = SerializedHeaderAndTransactions {
            serialized_block_header: serialized_block_header.clone(),
            serialized_transactions: serialized_transactions.clone(),
        };
        let bytes = bcs::to_bytes(&serialized_header_and_transactions)
            .map_err(ConsensusError::SerializationFailure)?;
        Ok(Self {
            serialized_block: Bytes::from(bytes),
            serialized_headers: vec![],
            serialized_shards: vec![],
            useful_headers_authors_bitmask: [0u64; 4],
            useful_shards_authors_bitmask: [0u64; 4],
        })
    }
}

impl TryFrom<BlockBundle> for SerializedBlockBundleParts {
    type Error = ConsensusError;
    fn try_from(block_bundle: BlockBundle) -> ConsensusResult<Self> {
        let (serialized_block_header, serialized_transactions) =
            block_bundle.verified_block.serialized();
        let serialized_header_and_transactions = SerializedHeaderAndTransactions {
            serialized_block_header: serialized_block_header.clone(),
            serialized_transactions: serialized_transactions.clone(),
        };
        let bytes = bcs::to_bytes(&serialized_header_and_transactions)
            .map_err(ConsensusError::SerializationFailure)?;
        let mut serialized_block_headers = vec![];
        for block_header in block_bundle.verified_headers.iter() {
            serialized_block_headers.push(block_header.serialized().clone());
        }
        Ok(Self {
            serialized_block: Bytes::from(bytes),
            serialized_headers: serialized_block_headers,
            serialized_shards: block_bundle.serialized_shards,
            useful_headers_authors_bitmask: authority_set_to_bitmask(
                &block_bundle.useful_headers_authors,
            ),
            useful_shards_authors_bitmask: authority_set_to_bitmask(
                &block_bundle.useful_shards_authors,
            ),
        })
    }
}

impl TryFrom<SerializedBlockBundleParts> for SerializedBlockBundle {
    type Error = ConsensusError;
    fn try_from(serialized_block_and_headers: SerializedBlockBundleParts) -> ConsensusResult<Self> {
        let bytes = bcs::to_bytes(&serialized_block_and_headers)
            .map_err(ConsensusError::SerializationFailure)?;
        Ok(Self {
            serialized_block_bundle: Bytes::from(bytes),
        })
    }
}

impl TryFrom<SerializedBlockBundle> for SerializedBlockBundleParts {
    type Error = ConsensusError;
    fn try_from(bundle: SerializedBlockBundle) -> ConsensusResult<Self> {
        bcs::from_bytes(&bundle.serialized_block_bundle)
            .map_err(ConsensusError::DeserializationFailure)
    }
}

impl TryFrom<VerifiedBlock> for SerializedBlockBundle {
    type Error = ConsensusError;
    fn try_from(verified_block: VerifiedBlock) -> ConsensusResult<Self> {
        SerializedBlockBundle::try_from(SerializedBlockBundleParts::try_from(verified_block)?)
    }
}

impl TryFrom<BlockBundle> for SerializedBlockBundle {
    type Error = ConsensusError;
    fn try_from(block_bundle: BlockBundle) -> ConsensusResult<Self> {
        SerializedBlockBundle::try_from(SerializedBlockBundleParts::try_from(block_bundle)?)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct SerializedTransactionsV1 {
    pub(crate) block_ref: BlockRef,
    pub(crate) serialized_transactions: Bytes,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct SerializedTransactionsV2 {
    pub(crate) transaction_ref: TransactionRef,
    pub(crate) serialized_transactions: Bytes,
}

#[cfg(test)]
mod tests {
    use rand::{seq::IteratorRandom, thread_rng};

    use super::*;
    use crate::TestBlockHeader;
    #[test]
    fn test_block_bundle_useful_authorities_set_bitmask_conversion() {
        let block = VerifiedBlock::new_for_test(TestBlockHeader::new(0u32, 0u8).build());
        // Generate a random sample of AuthorityIndex values (from 0..=255).
        let mut rng = thread_rng();
        let useful_authorities: BTreeSet<AuthorityIndex> = (0u8..=255)
            .choose_multiple(&mut rng, 50) // pick 50 random distinct authorities
            .into_iter()
            .map(AuthorityIndex::from)
            .collect();

        let block_bundle = BlockBundle {
            verified_block: block,
            verified_headers: vec![],
            serialized_shards: vec![],
            useful_headers_authors: useful_authorities.clone(),
            useful_shards_authors: useful_authorities.clone(),
        };
        let serialized_bundle = SerializedBlockBundle::try_from(block_bundle).unwrap();
        let serialized_bundle_parts =
            SerializedBlockBundleParts::try_from(serialized_bundle).unwrap();
        let converted_useful_authorities = serialized_bundle_parts.useful_headers_authors();
        assert_eq!(useful_authorities, converted_useful_authorities);
    }
}
