// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use async_trait::async_trait;
use bytes::Bytes;
use futures::stream;
use parking_lot::Mutex;
use starfish_config::AuthorityIndex;

use crate::{
    Round,
    block_header::{BlockRef, VerifiedBlockHeader},
    commit::{CommitRange, TrustedCommit},
    error::ConsensusResult,
    network::{
        BlockBundleStream, BlockStream, NetworkService, SerializedBlock, SerializedBlockBundle,
    },
};

pub(crate) struct TestService {
    pub(crate) handle_subscribed_block: Vec<(AuthorityIndex, SerializedBlock)>,
    pub(crate) handle_fetch_block_headers: Vec<(AuthorityIndex, Vec<BlockRef>)>,
    pub(crate) handle_fetch_blocks: Vec<(AuthorityIndex, Vec<BlockRef>)>,
    pub(crate) handle_subscribe_blocks: Vec<(AuthorityIndex, Round)>,
    pub(crate) handle_fetch_commits: Vec<(AuthorityIndex, CommitRange)>,
    pub(crate) own_blocks: Vec<SerializedBlock>,
}

impl TestService {
    pub(crate) fn new() -> Self {
        Self {
            handle_subscribed_block: Vec::new(),
            handle_fetch_block_headers: Vec::new(),
            handle_fetch_blocks: Vec::new(),
            handle_subscribe_blocks: Vec::new(),
            handle_fetch_commits: Vec::new(),
            own_blocks: Vec::new(),
        }
    }

    #[cfg_attr(msim, allow(dead_code))]
    pub(crate) fn add_own_blocks(&mut self, blocks: Vec<SerializedBlock>) {
        self.own_blocks.extend(blocks);
    }
}

#[async_trait]
impl NetworkService for Mutex<TestService> {
    async fn handle_subscribed_block(
        &self,
        peer: AuthorityIndex,
        serialized_block: SerializedBlock,
    ) -> ConsensusResult<()> {
        let mut state = self.lock();
        state.handle_subscribed_block.push((peer, serialized_block));
        Ok(())
    }

    async fn handle_subscribed_block_bundle(
        &self,
        _peer: AuthorityIndex,
        _serialized_block_bundle: SerializedBlockBundle,
    ) -> ConsensusResult<()> {
        unimplemented!("Unimplemented")
    }

    async fn handle_subscribe_blocks(
        &self,
        peer: AuthorityIndex,
        last_received: Round,
    ) -> ConsensusResult<BlockStream> {
        let mut state = self.lock();
        state.handle_subscribe_blocks.push((peer, last_received));
        let own_blocks = state
            .own_blocks
            .iter()
            // Let index in own_blocks be the round, and skip blocks <= last_received round.
            .skip(last_received as usize + 1)
            .cloned()
            .collect::<Vec<_>>();
        Ok(Box::pin(stream::iter(own_blocks)))
    }

    async fn handle_subscribe_block_bundles_request(
        &self,
        _peer: AuthorityIndex,
        _last_received: Round,
    ) -> ConsensusResult<BlockBundleStream> {
        unimplemented!("Unimplemented");
    }

    async fn handle_fetch_block_headers(
        &self,
        peer: AuthorityIndex,
        block_refs: Vec<BlockRef>,
        _highest_accepted_rounds: Vec<Round>,
    ) -> ConsensusResult<Vec<Bytes>> {
        self.lock()
            .handle_fetch_block_headers
            .push((peer, block_refs));
        Ok(vec![])
    }

    async fn handle_fetch_blocks(
        &self,
        peer: AuthorityIndex,
        block_refs: Vec<BlockRef>,
        _highest_accepted_rounds: Vec<Round>,
    ) -> ConsensusResult<Vec<Bytes>> {
        self.lock().handle_fetch_blocks.push((peer, block_refs));
        Ok(vec![])
    }

    async fn handle_fetch_commits(
        &self,
        peer: AuthorityIndex,
        commit_range: CommitRange,
    ) -> ConsensusResult<(Vec<TrustedCommit>, Vec<VerifiedBlockHeader>)> {
        self.lock().handle_fetch_commits.push((peer, commit_range));
        Ok((vec![], vec![]))
    }

    async fn handle_fetch_latest_blocks(
        &self,
        _peer: AuthorityIndex,
        _authorities: Vec<AuthorityIndex>,
    ) -> ConsensusResult<Vec<Bytes>> {
        unimplemented!("Unimplemented")
    }

    async fn handle_get_latest_rounds(
        &self,
        _peer: AuthorityIndex,
    ) -> ConsensusResult<(Vec<Round>, Vec<Round>)> {
        unimplemented!("Unimplemented")
    }

    async fn handle_fetch_transactions(
        &self,
        _peer: AuthorityIndex,
        _block_refs: Vec<BlockRef>,
    ) -> ConsensusResult<Vec<Bytes>> {
        unimplemented!("Unimplemented")
    }
}
