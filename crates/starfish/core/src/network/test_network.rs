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
    commit_syncer::CommitSyncType,
    encoder::ShardEncoder,
    error::ConsensusResult,
    network::{BlockBundleStream, NetworkService, SerializedBlockBundle},
    transaction_ref::GenericTransactionRef,
};

pub(crate) struct TestService {
    pub(crate) handle_subscribed_block_bundle: Vec<(AuthorityIndex, SerializedBlockBundle)>,
    pub(crate) handle_subscribed_block_bundle_requests: Vec<(AuthorityIndex, Round)>,
    pub(crate) handle_fetch_block_headers: Vec<(AuthorityIndex, Vec<BlockRef>)>,
    pub(crate) handle_fetch_commits: Vec<(AuthorityIndex, CommitRange)>,
    pub(crate) own_block_bundles: Vec<SerializedBlockBundle>,
}

impl TestService {
    pub(crate) fn new() -> Self {
        Self {
            own_block_bundles: Vec::new(),
            handle_subscribed_block_bundle: Vec::new(),
            handle_subscribed_block_bundle_requests: Vec::new(),
            handle_fetch_block_headers: Vec::new(),
            handle_fetch_commits: Vec::new(),
        }
    }

    #[cfg_attr(msim, allow(dead_code))]
    pub(crate) fn add_own_blocks(&mut self, blocks: Vec<SerializedBlockBundle>) {
        self.own_block_bundles.extend(blocks);
    }
}

#[async_trait]
impl NetworkService for Mutex<TestService> {
    async fn handle_subscribed_block_bundle(
        &self,
        peer: AuthorityIndex,
        serialized_block_bundle: SerializedBlockBundle,
        _encoder: &mut Box<dyn ShardEncoder + Send + Sync>,
    ) -> ConsensusResult<()> {
        let mut state = self.lock();
        state
            .handle_subscribed_block_bundle
            .push((peer, serialized_block_bundle));
        Ok(())
    }

    async fn handle_subscribe_block_bundles_request(
        &self,
        peer: AuthorityIndex,
        last_received: Round,
    ) -> ConsensusResult<BlockBundleStream> {
        let mut state = self.lock();
        state
            .handle_subscribed_block_bundle_requests
            .push((peer, last_received));
        let own_blocks = state
            .own_block_bundles
            .iter()
            // Let index in own_blocks be the round, and skip blocks <= last_received round.
            .skip(last_received as usize + 1)
            .cloned()
            .collect::<Vec<_>>();
        Ok(Box::pin(stream::iter(own_blocks)))
    }

    async fn handle_fetch_headers(
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

    async fn handle_fetch_commits(
        &self,
        peer: AuthorityIndex,
        commit_range: CommitRange,
        _commit_sync_type: CommitSyncType,
    ) -> ConsensusResult<(Vec<TrustedCommit>, Vec<VerifiedBlockHeader>)> {
        self.lock().handle_fetch_commits.push((peer, commit_range));
        Ok((vec![], vec![]))
    }

    async fn handle_fetch_commits_and_transactions(
        &self,
        _peer: AuthorityIndex,
        _commit_range: CommitRange,
    ) -> ConsensusResult<(Vec<Bytes>, Vec<Bytes>, Vec<Bytes>)> {
        unimplemented!("Unimplemented")
    }

    async fn handle_fetch_latest_block_headers(
        &self,
        _peer: AuthorityIndex,
        _authorities: Vec<AuthorityIndex>,
    ) -> ConsensusResult<Vec<Bytes>> {
        unimplemented!("Unimplemented")
    }

    async fn handle_fetch_transactions(
        &self,
        _peer: AuthorityIndex,
        _block_refs: Vec<GenericTransactionRef>,
        _fetch_mode: crate::network::TransactionFetchMode,
    ) -> ConsensusResult<Vec<Bytes>> {
        unimplemented!("Unimplemented")
    }
}
