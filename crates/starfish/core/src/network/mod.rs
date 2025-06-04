// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This module defines the network interface, and provides network
//! implementations for the consensus protocol.
//!
//! Having an abstract network interface allows
//! - simplying the semantics of sending data and serving requests over the
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

use std::{pin::Pin, time::Duration};

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

/// A stream of serialized filtered blocks returned over the network.
pub(crate) type BlockStream = Pin<Box<dyn Stream<Item = SerializedBlock> + Send>>;

/// Network client for communicating with peers.
///
/// NOTE: the timeout parameters help saving resources at client and potentially
/// server. But it is up to the server implementation if the timeout is honored.
/// - To bound server resources, server should implement own timeout for
///   incoming requests.
#[async_trait]
pub(crate) trait NetworkClient: Send + Sync + Sized + 'static {
    /// Subscribes to blocks from a peer after last_received round.
    async fn subscribe_blocks(
        &self,
        peer: AuthorityIndex,
        last_received: Round,
        timeout: Duration,
    ) -> ConsensusResult<BlockStream>;

    /// Fetches serialized `SerializedBlocks` from a peer. It also might
    /// return additional ancestor blocks of the requested blocks according
    /// to the provided `highest_accepted_rounds`. The
    /// `highest_accepted_rounds` length should be equal to the committee
    /// size. If `highest_accepted_rounds` is empty then it will be simply
    /// ignored.
    async fn fetch_blocks(
        &self,
        peer: AuthorityIndex,
        block_refs: Vec<BlockRef>,
        highest_accepted_rounds: Vec<Round>,
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
    /// Returns a tuple of both the serialized commits, and serialized blocks
    /// that contain votes certifying the last commit.
    async fn fetch_commits(
        &self,
        peer: AuthorityIndex,
        commit_range: CommitRange,
        timeout: Duration,
    ) -> ConsensusResult<(Vec<Bytes>, Vec<Bytes>)>;

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
    /// Handles the block sent from the peer via subscription stream. Peer value
    /// can be trusted to be a valid authority index. But serialized_block
    /// must be verified before its contents are trusted.
    async fn handle_subscribed_block(
        &self,
        peer: AuthorityIndex,
        serialized_block: SerializedBlock,
    ) -> ConsensusResult<()>;

    /// Handles the subscription request from the peer.
    /// A stream of newly proposed blocks is returned to the peer.
    /// The stream continues until the end of epoch, peer unsubscribes, or a
    /// network error / crash occurs.
    async fn handle_subscribe_blocks(
        &self,
        peer: AuthorityIndex,
        last_received: Round,
    ) -> ConsensusResult<BlockStream>;

    /// Handles the request to fetch block headers by references from the peer.
    async fn handle_fetch_block_headers(
        &self,
        peer: AuthorityIndex,
        block_refs: Vec<BlockRef>,
        highest_accepted_rounds: Vec<Round>,
    ) -> ConsensusResult<Vec<Bytes>>;

    /// Handles the request to fetch blocks by references from the peer.
    /// The function returns Vec<Bytes>. Each element is a serialization of
    /// header and transactions of a block.
    async fn handle_fetch_blocks(
        &self,
        peer: AuthorityIndex,
        block_refs: Vec<BlockRef>,
        highest_accepted_rounds: Vec<Round>,
    ) -> ConsensusResult<Vec<Bytes>>;

    /// Handles the request to fetch commits by index range from the peer.
    async fn handle_fetch_commits(
        &self,
        peer: AuthorityIndex,
        commit_range: CommitRange,
    ) -> ConsensusResult<(Vec<TrustedCommit>, Vec<VerifiedBlockHeader>)>;

    /// Handles the request to fetch the latest block for the provided
    /// `authorities`.
    async fn handle_fetch_latest_blocks(
        &self,
        peer: AuthorityIndex,
        authorities: Vec<AuthorityIndex>,
    ) -> ConsensusResult<Vec<Bytes>>;

    /// Handles the request to get the latest received & accepted rounds of all
    /// authorities.
    async fn handle_get_latest_rounds(
        &self,
        peer: AuthorityIndex,
    ) -> ConsensusResult<(Vec<Round>, Vec<Round>)>;
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

impl SerializedHeaderAndTransactions {
    #[cfg(test)]
    pub(crate) fn new_for_test(bytes: Bytes) -> Self {
        Self {
            serialized_block_header: bytes.clone(),
            serialized_transactions: bytes,
        }
    }
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
        bcs::from_bytes(&serialized_block.serialized_block).map_err(ConsensusError::MalformedBlock)
    }
}
