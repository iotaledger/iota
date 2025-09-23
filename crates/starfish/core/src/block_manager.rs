// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, BTreeSet, btree_map::Entry},
    sync::Arc,
    time::Instant,
};

use iota_metrics::monitored_scope;
use itertools::Itertools as _;
use parking_lot::RwLock;
use starfish_config::AuthorityIndex;
use tracing::{debug, warn};

use crate::{
    Round,
    block_header::{
        BlockHeaderAPI, BlockRef, BlockTimestampMs, VerifiedBlock, VerifiedBlockHeader,
    },
    context::Context,
    dag_state::DagState,
    error::{ConsensusError, ConsensusResult},
};

struct SuspendedBlockHeader {
    block_header: VerifiedBlockHeader,
    missing_ancestors: BTreeSet<BlockRef>,
    timestamp: Instant,
}

impl SuspendedBlockHeader {
    fn new(block: VerifiedBlockHeader, missing_ancestors: BTreeSet<BlockRef>) -> Self {
        Self {
            block_header: block,
            missing_ancestors,
            timestamp: Instant::now(),
        }
    }
}

/// Block manager suspends incoming blocks until they are connected to the
/// existing graph, returning newly connected blocks.
/// TODO: As it is possible to have Byzantine validators who produce Blocks
/// without valid causal history we need to make sure that BlockManager takes
/// care of that and avoid OOM (Out Of Memory) situations.
pub(crate) struct BlockManager {
    context: Arc<Context>,
    dag_state: Arc<RwLock<DagState>>,

    /// Keeps all the suspended block headers. A suspended block header is a
    /// header that is missing part of its causal history and thus can't be
    /// immediately processed. A block header will remain in this map until
    /// all its causal history has been successfully processed.
    suspended_block_headers: BTreeMap<BlockRef, SuspendedBlockHeader>,
    /// Keeps full blocks for suspended block headers
    /// TODO: this set can grow to become too big, need to add some eviction
    /// mechanism
    suspended_blocks: BTreeMap<BlockRef, VerifiedBlock>,

    /// A map that keeps all the blocks that we are missing (keys) and the
    /// corresponding blocks that reference the missing blocks as ancestors
    /// and need them to get unsuspended. It is possible for a missing
    /// dependency (key) to be a suspended block, so the block has been
    /// already fetched but it self is still missing some of its ancestors to be
    /// processed.
    missing_ancestors: BTreeMap<BlockRef, BTreeSet<BlockRef>>,
    /// A map of currently missing blocks to the set of authorities expected
    /// to have them available locally. This set is approximated based on the
    /// block's author and the authors of its direct children.
    /// A block is considered missing if it appears in `missing_ancestors`
    /// and has not yet been accepted or fetched. Blocks already stored or
    /// present in `suspended_blocks` are excluded.
    missing_block_headers: BTreeMap<BlockRef, BTreeSet<AuthorityIndex>>,
    /// A vector that holds a tuple of (lowest_round, highest_round) of received
    /// blocks per authority. This is used for metrics reporting purposes
    /// and resets during restarts.
    received_block_rounds: Vec<Option<(Round, Round)>>,
}

impl BlockManager {
    pub(crate) fn new(context: Arc<Context>, dag_state: Arc<RwLock<DagState>>) -> Self {
        let committee_size = context.committee.size();
        Self {
            context,
            dag_state,
            suspended_block_headers: BTreeMap::new(),
            suspended_blocks: BTreeMap::new(),
            missing_ancestors: BTreeMap::new(),
            missing_block_headers: BTreeMap::new(),
            received_block_rounds: vec![None; committee_size],
        }
    }

    /// Does all the same things as try_accept_block_headers and additionally
    /// saves blocks with transaction data into recent_blocks in DagState
    #[tracing::instrument(skip_all)]
    pub(crate) fn try_accept_blocks(
        &mut self,
        blocks: Vec<VerifiedBlock>,
    ) -> (Vec<VerifiedBlockHeader>, BTreeSet<BlockRef>) {
        let _s = monitored_scope("BlockManager::try_accept_blocks");
        let block_headers: Vec<_> = blocks
            .iter()
            .map(|b| b.verified_block_header.clone())
            .collect();
        let (accepted_block_headers, missing_block_headers) =
            self.try_accept_block_headers_internal(block_headers);

        let block_refs = blocks
            .iter()
            .map(|b| b.verified_block_header.reference())
            .collect();
        let exists = self.dag_state.read().contains_block_headers(block_refs);
        for (i, block) in blocks.into_iter().enumerate() {
            if exists[i] {
                self.dag_state
                    .write()
                    .add_transactions(block.verified_transactions);
            } else {
                self.suspended_blocks.insert(block.reference(), block);
            }
        }

        (accepted_block_headers, missing_block_headers)
    }

    /// Tries to accept the provided block headers assuming that all their
    /// causal history exists. The method returns all the block headers that
    /// have been successfully processed in round ascending order, that
    /// includes also previously suspended block headers that have now been
    /// able to get accepted. Method also returns a set with the missing
    /// ancestor block headers.
    #[tracing::instrument(skip_all)]
    pub(crate) fn try_accept_block_headers(
        &mut self,
        block_headers: Vec<VerifiedBlockHeader>,
    ) -> (Vec<VerifiedBlockHeader>, BTreeSet<BlockRef>) {
        let _s = monitored_scope("BlockManager::try_accept_block_headers");
        // Headers are added through synchronizer, commit syncer and cordial
        // dissemination.
        self.try_accept_block_headers_internal(block_headers)
    }

    /// Attempts to accept the provided blocks.
    fn try_accept_block_headers_internal(
        &mut self,
        mut block_headers: Vec<VerifiedBlockHeader>,
    ) -> (Vec<VerifiedBlockHeader>, BTreeSet<BlockRef>) {
        let _s = monitored_scope("BlockManager::try_accept_block_headers_internal");

        block_headers.sort_by_key(|b| b.round());
        debug!(
            "Trying to accept block headers: {}",
            block_headers
                .iter()
                .map(|b| b.reference().to_string())
                .join(",")
        );

        let mut accepted_block_headers = vec![];
        let mut missing_block_headers = BTreeSet::new();

        for block_header in block_headers {
            self.update_block_received_metrics(&block_header);

            // Try to accept the input block.
            let block_ref = block_header.reference();

            let mut to_verify_timestamps_and_accept = vec![];

            match self.try_accept_one_block_header(block_header) {
                TryAcceptResult::Accepted(block_header) => {
                    to_verify_timestamps_and_accept.push(block_header);
                }
                TryAcceptResult::Suspended(ancestors_to_fetch) => {
                    debug!(
                        "Missing ancestors to fetch for block {block_ref}: {}",
                        ancestors_to_fetch.iter().map(|b| b.to_string()).join(",")
                    );
                    missing_block_headers.extend(ancestors_to_fetch);
                    continue;
                }
                TryAcceptResult::Processed => continue,
            };

            // If the block is accepted, try to unsuspend its children blocks if any.
            let unsuspended_blocks = self.try_unsuspend_children_blocks(block_ref);
            to_verify_timestamps_and_accept.extend(unsuspended_blocks);

            // Verify block timestamps
            let blocks_to_accept =
                self.verify_block_timestamps_and_accept(to_verify_timestamps_and_accept);
            accepted_block_headers.extend(blocks_to_accept);
        }

        self.update_stats(missing_block_headers.len() as u64);

        // check if we already have blocks for this accepted header. If yes, add them to
        // dag

        for block_header in accepted_block_headers.iter() {
            if let Some(block) = self.suspended_blocks.remove(&block_header.reference()) {
                // for this accepted header we already have a block, so we add it to dag_state
                self.dag_state
                    .write()
                    .add_transactions(block.verified_transactions);
            }
        }

        // Figure out the new missing blocks
        (accepted_block_headers, missing_block_headers)
    }

    /// Tries to find the provided block_refs in DagState and BlockManager,
    /// and returns missing block refs. Used to test behavior in case of
    /// equivocation.
    #[cfg(test)]
    pub(crate) fn try_find_blocks(&mut self, block_refs: Vec<BlockRef>) -> BTreeSet<BlockRef> {
        let _s = monitored_scope("BlockManager::try_find_blocks");

        let mut block_refs = block_refs;

        if block_refs.is_empty() {
            return BTreeSet::new();
        }

        block_refs.sort_by_key(|b| b.round);

        debug!(
            "Trying to find blocks: {}",
            block_refs.iter().map(|b| b.to_string()).join(",")
        );

        let mut missing_blocks = BTreeSet::new();

        for (found, block_ref) in self
            .dag_state
            .read()
            .contains_block_headers(block_refs.clone())
            .into_iter()
            .zip(block_refs.iter())
        {
            if found || self.suspended_block_headers.contains_key(block_ref) {
                continue;
            }
            // Fetches the block if it is not in dag state or suspended.
            missing_blocks.insert(*block_ref);
            if self
                .missing_block_headers
                .insert(*block_ref, BTreeSet::from([block_ref.author]))
                .is_none()
            {
                // We want to report this as a missing ancestor even if there is no block that
                // is actually references it right now.
                self.missing_ancestors.entry(*block_ref).or_default();

                self.context
                    .metrics
                    .node_metrics
                    .block_manager_missing_blocks_by_authority
                    .with_label_values(&[self.context.authority_hostname(block_ref.author)])
                    .inc();
            }
        }

        let metrics = &self.context.metrics.node_metrics;
        metrics
            .missing_blocks_total
            .inc_by(missing_blocks.len() as u64);
        metrics
            .block_manager_missing_blocks
            .set(self.missing_block_headers.len() as i64);

        missing_blocks
    }
    /// Verifies a block w.r.t. ancestor blocks.
    /// This is called after a block has complete causal history locally,
    /// and is ready to be accepted into the DAG.
    ///
    /// Caller must make sure ancestors corresponse to block.ancestors() 1-to-1,
    /// in the same order.
    fn check_ancestors(
        &self,
        block: &VerifiedBlockHeader,
        ancestors: &[VerifiedBlockHeader],
    ) -> ConsensusResult<()> {
        assert_eq!(block.ancestors().len(), ancestors.len());
        // This checks the invariant that block timestamp >= max ancestor timestamp.
        let mut max_timestamp_ms = BlockTimestampMs::MIN;
        for (ancestor_ref, ancestor_block) in block.ancestors().iter().zip(ancestors.iter()) {
            assert_eq!(ancestor_ref, &ancestor_block.reference());
            max_timestamp_ms = max_timestamp_ms.max(ancestor_block.timestamp_ms());
        }
        if max_timestamp_ms > block.timestamp_ms() {
            return Err(ConsensusError::InvalidBlockTimestamp {
                max_timestamp_ms,
                block_timestamp_ms: block.timestamp_ms(),
            });
        }
        Ok(())
    }
    // TODO: remove once timestamping is refactored to the new approach.
    // Verifies each block's timestamp based on its ancestors, and persists in store
    // all the valid blocks that should be accepted. Method returns the accepted
    // and persisted blocks.
    fn verify_block_timestamps_and_accept(
        &mut self,
        unsuspended_blocks: impl IntoIterator<Item = VerifiedBlockHeader>,
    ) -> Vec<VerifiedBlockHeader> {
        // Try to verify the block and its children for timestamp, with ancestor blocks.
        let mut blocks_to_accept: BTreeMap<BlockRef, VerifiedBlockHeader> = BTreeMap::new();
        let mut blocks_to_reject: BTreeMap<BlockRef, VerifiedBlockHeader> = BTreeMap::new();
        {
            'block: for b in unsuspended_blocks {
                let ancestors = self.dag_state.read().get_block_headers(b.ancestors());
                assert_eq!(b.ancestors().len(), ancestors.len());
                let mut ancestor_blocks = vec![];
                'ancestor: for (ancestor_ref, found) in
                    b.ancestors().iter().zip(ancestors.into_iter())
                {
                    if let Some(found_block) = found {
                        // This invariant should be guaranteed by DagState.
                        assert_eq!(ancestor_ref, &found_block.reference());
                        ancestor_blocks.push(found_block);
                        continue 'ancestor;
                    }
                    // blocks_to_accept have not been added to DagState yet, but they
                    // can appear in ancestors.
                    if blocks_to_accept.contains_key(ancestor_ref) {
                        ancestor_blocks.push(blocks_to_accept[ancestor_ref].clone());
                        continue 'ancestor;
                    }
                    // If an ancestor is already rejected, reject this block as well.
                    if blocks_to_reject.contains_key(ancestor_ref) {
                        blocks_to_reject.insert(b.reference(), b);
                        continue 'block;
                    }

                    {
                        panic!(
                            "Unsuspended block {b:?} has a missing ancestor! Ancestor not found in DagState: {ancestor_ref:?}"
                        );
                    }
                }
                if let Err(e) = self.check_ancestors(&b, &ancestor_blocks) {
                    warn!("Block {:?} failed to verify ancestors: {}", b, e);
                    blocks_to_reject.insert(b.reference(), b);
                } else {
                    blocks_to_accept.insert(b.reference(), b);
                }
            }
        }

        // TODO: report blocks_to_reject to peers.
        for (block_ref, block) in blocks_to_reject {
            self.context
                .metrics
                .node_metrics
                .invalid_block_headers
                .with_label_values(&[
                    self.context.authority_hostname(block_ref.author),
                    "accept_block",
                    "InvalidAncestors",
                ])
                .inc();
            warn!("Invalid block {:?} is rejected", block);
        }

        let blocks_to_accept = blocks_to_accept.values().cloned().collect::<Vec<_>>();

        // Insert the accepted blocks into DAG state so future blocks including them as
        // ancestors do not get suspended.
        self.dag_state
            .write()
            .accept_block_headers(blocks_to_accept.clone());

        blocks_to_accept
    }

    /// Tries to accept the provided block. To accept a block its ancestors must
    /// have been already successfully accepted. If block is accepted then
    /// Some result is returned. None is returned when either the block is
    /// suspended or the block has been already accepted before.
    fn try_accept_one_block_header(
        &mut self,
        block_header: VerifiedBlockHeader,
    ) -> TryAcceptResult {
        let block_ref = block_header.reference();
        let mut missing_ancestors = BTreeSet::new();
        let mut ancestors_to_fetch = BTreeSet::new();
        let dag_state = self.dag_state.read();

        // If block has been already received and suspended, or already processed and
        // stored, or is a genesis block, then skip it.
        if self.suspended_block_headers.contains_key(&block_ref)
            || dag_state.contains_block_header(&block_ref)
        {
            self.context
                .metrics
                .node_metrics
                .block_manager_filtered_processed_headers_by_authority
                .with_label_values(&[self.context.authority_hostname(block_ref.author)])
                .inc();
            return TryAcceptResult::Processed;
        }

        let ancestors = block_header.ancestors().to_vec();

        // make sure that we have all the required ancestors in store
        for (found, ancestor) in dag_state
            .contains_block_headers(ancestors.clone())
            .into_iter()
            .zip(ancestors.iter())
        {
            if !found {
                missing_ancestors.insert(*ancestor);

                // mark the block as having missing ancestors
                self.missing_ancestors
                    .entry(*ancestor)
                    .or_default()
                    .insert(block_ref);

                self.context
                    .metrics
                    .node_metrics
                    .block_manager_missing_ancestors_by_authority
                    .with_label_values(&[self.context.authority_hostname(ancestor.author)])
                    .inc();

                // Add the ancestor to the missing blocks set only if it doesn't already exist
                // in the suspended blocks - meaning that we already have its
                // payload.
                if !self.suspended_block_headers.contains_key(ancestor) {
                    // Fetches the block if it is not in dag state or suspended.
                    ancestors_to_fetch.insert(*ancestor);
                    // We also want to keep track of the authorities that have this block.
                    // This block could be already missing, so we just update the set  of
                    // authorities who have it.
                    let entry = self.missing_block_headers.entry(*ancestor);
                    match entry {
                        Entry::Vacant(v) => {
                            v.insert(BTreeSet::from([ancestor.author, block_ref.author]));
                            self.context
                                .metrics
                                .node_metrics
                                .block_manager_missing_blocks_by_authority
                                .with_label_values(&[self
                                    .context
                                    .authority_hostname(ancestor.author)])
                                .inc();
                        }
                        Entry::Occupied(mut o) => {
                            o.get_mut().insert(block_ref.author);
                        }
                    }
                }
            }
        }

        // Remove the block ref from the `missing_blocks` - if exists - since we now
        // have received the block. The block might still get suspended, but we
        // won't report it as missing in order to not re-fetch.
        self.missing_block_headers.remove(&block_header.reference());

        if !missing_ancestors.is_empty() {
            self.context
                .metrics
                .node_metrics
                .block_suspensions
                .with_label_values(&[self.context.authority_hostname(block_header.author())])
                .inc();
            self.suspended_block_headers.insert(
                block_ref,
                SuspendedBlockHeader::new(block_header, missing_ancestors),
            );
            return TryAcceptResult::Suspended(ancestors_to_fetch);
        }

        TryAcceptResult::Accepted(block_header)
    }

    /// Given an accepted block `accepted_block` it attempts to accept all the
    /// suspended children blocks assuming such exist. All the unsuspended /
    /// accepted blocks are returned as a vector in causal order.
    fn try_unsuspend_children_blocks(
        &mut self,
        accepted_block: BlockRef,
    ) -> Vec<VerifiedBlockHeader> {
        let mut unsuspended_blocks = vec![];
        let mut to_process_blocks = vec![accepted_block];

        while let Some(block_ref) = to_process_blocks.pop() {
            // And try to check if its direct children can be unsuspended
            if let Some(block_refs_with_missing_deps) = self.missing_ancestors.remove(&block_ref) {
                for r in block_refs_with_missing_deps {
                    // For each dependency try to unsuspend it. If that's successful then we add it
                    // to the queue so we can recursively try to unsuspend its
                    // children.
                    if let Some(block) = self.try_unsuspend_block(&r, &block_ref) {
                        to_process_blocks.push(block.block_header.reference());
                        unsuspended_blocks.push(block);
                    }
                }
            }
        }

        let now = Instant::now();

        // Report the unsuspended blocks
        for block in &unsuspended_blocks {
            self.context
                .metrics
                .node_metrics
                .block_unsuspensions
                .with_label_values(&[self.context.authority_hostname(block.block_header.author())])
                .inc();
            self.context
                .metrics
                .node_metrics
                .suspended_block_time
                .with_label_values(&[self.context.authority_hostname(block.block_header.author())])
                .observe(now.saturating_duration_since(block.timestamp).as_secs_f64());
        }

        unsuspended_blocks
            .into_iter()
            .map(|block| block.block_header)
            .collect()
    }

    /// Attempts to unsuspend a block by checking its ancestors and removing the
    /// `accepted_dependency` by its local set. If there is no missing
    /// dependency then this block can be unsuspended immediately and is removed
    /// from the `suspended_blocks` map.
    fn try_unsuspend_block(
        &mut self,
        block_ref: &BlockRef,
        accepted_dependency: &BlockRef,
    ) -> Option<SuspendedBlockHeader> {
        let block = self
            .suspended_block_headers
            .get_mut(block_ref)
            .expect("Block should be in suspended map");

        assert!(
            block.missing_ancestors.remove(accepted_dependency),
            "Block reference {} should be present in missing dependencies of {:?}",
            block_ref,
            block.block_header
        );

        if block.missing_ancestors.is_empty() {
            // we have no missing dependency, so we unsuspend the block and return it
            return self.suspended_block_headers.remove(block_ref);
        }
        None
    }

    /// Returns all the block headers that are currently missing and needed in
    /// order to accept suspended block headers. For each block reference it
    /// returns the set of authorities who have this block header.
    pub(crate) fn missing_block_headers(&self) -> BTreeMap<BlockRef, BTreeSet<AuthorityIndex>> {
        self.missing_block_headers.clone()
    }

    /// Returns all the block refs that are currently missing.
    #[cfg(test)]
    pub(crate) fn missing_block_refs(&self) -> BTreeSet<BlockRef> {
        self.missing_block_headers.keys().cloned().collect()
    }

    fn update_stats(&mut self, missing_blocks: u64) {
        let metrics = &self.context.metrics.node_metrics;
        metrics.missing_blocks_total.inc_by(missing_blocks);
        metrics
            .block_manager_suspended_blocks
            .set(self.suspended_block_headers.len() as i64);
        metrics
            .block_manager_missing_ancestors
            .set(self.missing_ancestors.len() as i64);
        metrics
            .block_manager_missing_blocks
            .set(self.missing_block_headers.len() as i64);
    }

    fn update_block_received_metrics(&mut self, block: &VerifiedBlockHeader) {
        let (min_round, max_round) =
            if let Some((curr_min, curr_max)) = self.received_block_rounds[block.author()] {
                (curr_min.min(block.round()), curr_max.max(block.round()))
            } else {
                (block.round(), block.round())
            };
        self.received_block_rounds[block.author()] = Some((min_round, max_round));

        self.context
            .metrics
            .node_metrics
            .lowest_verified_authority_round
            .with_label_values(&[self.context.authority_hostname(block.author())])
            .set(min_round.into());
        self.context
            .metrics
            .node_metrics
            .highest_verified_authority_round
            .with_label_values(&[self.context.authority_hostname(block.author())])
            .set(max_round.into());
    }

    /// Checks if block manager is empty.
    #[cfg(test)]
    pub(crate) fn is_empty(&self) -> bool {
        self.suspended_block_headers.is_empty()
            && self.missing_ancestors.is_empty()
            && self.missing_block_headers.is_empty()
    }

    /// Returns all the suspended blocks refs whose causal history we miss hence
    /// we can't accept them yet.
    #[cfg(test)]
    fn suspended_blocks_refs(&self) -> BTreeSet<BlockRef> {
        self.suspended_block_headers.keys().cloned().collect()
    }
}

// Result of trying to accept one block.
enum TryAcceptResult {
    // The block is accepted. Wraps the block itself.
    Accepted(VerifiedBlockHeader),
    // The block is suspended. Wraps ancestors to be fetched.
    Suspended(BTreeSet<BlockRef>),
    // The block has been processed before and already exists in BlockManager (and is suspended)
    // or in DagState (so has been already accepted). No further processing has been done at
    // this point.
    Processed,
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeSet, sync::Arc};

    use parking_lot::RwLock;
    use rand::{SeedableRng, prelude::StdRng, seq::SliceRandom};
    use starfish_config::AuthorityIndex;

    use crate::{
        TestBlockHeader,
        block_header::{BlockHeaderAPI, BlockRef, BlockTimestampMs, VerifiedBlockHeader},
        block_manager::BlockManager,
        context::Context,
        dag_state::DagState,
        error::ConsensusError,
        storage::mem_store::MemStore,
        test_dag_builder::DagBuilder,
    };

    #[tokio::test]
    async fn suspend_blocks_with_missing_ancestors() {
        // GIVEN
        let (context, _key_pairs) = Context::new_for_test(4);
        let context = Arc::new(context);
        let store = Arc::new(MemStore::new());
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store.clone())));

        let mut block_manager = BlockManager::new(context.clone(), dag_state);

        // create a DAG
        let mut dag_builder = DagBuilder::new(context.clone());
        dag_builder
            .layers(1..=2) // 2 rounds
            .authorities(vec![
                AuthorityIndex::new_for_test(0),
                AuthorityIndex::new_for_test(2),
            ]) // Create equivocating blocks for 2 authorities
            .equivocate(3)
            .build();

        // Take only the blocks of round 2 and try to accept them
        let round_2_block_headers = dag_builder
            .block_headers
            .into_iter()
            .filter_map(|(_, block_header)| (block_header.round() == 2).then_some(block_header))
            .collect::<Vec<VerifiedBlockHeader>>();

        // WHEN
        let (accepted_blocks, missing) =
            block_manager.try_accept_block_headers(round_2_block_headers.clone());

        // THEN
        assert!(accepted_blocks.is_empty());

        // AND the returned missing ancestors should be the same as the provided block
        // ancestors
        let missing_block_refs = round_2_block_headers.first().unwrap().ancestors();
        let missing_block_refs = missing_block_refs.iter().cloned().collect::<BTreeSet<_>>();
        assert_eq!(missing, missing_block_refs);

        // AND the missing blocks are the parents of the round 2 blocks. Since this is a
        // fully connected DAG taking the ancestors of the first element
        // suffices.
        assert_eq!(block_manager.missing_block_refs(), missing_block_refs);

        // AND suspended blocks should return the round_2_blocks
        assert_eq!(
            block_manager.suspended_blocks_refs(),
            round_2_block_headers
                .into_iter()
                .map(|block_header| block_header.reference())
                .collect::<BTreeSet<_>>()
        );

        // AND each missing block should be known to all authorities
        let known_by_manager = block_manager
            .missing_block_headers()
            .iter()
            .next()
            .expect("We should expect at least two elements there")
            .1
            .clone();
        assert_eq!(
            known_by_manager,
            context
                .committee
                .authorities()
                .map(|(a, _)| a)
                .collect::<BTreeSet<_>>()
        );
    }

    #[tokio::test]
    async fn try_accept_block_returns_missing_blocks() {
        let (context, _key_pairs) = Context::new_for_test(4);
        let context = Arc::new(context);
        let store = Arc::new(MemStore::new());
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store.clone())));

        let mut block_manager = BlockManager::new(context.clone(), dag_state);

        // create a DAG
        let mut dag_builder = DagBuilder::new(context.clone());
        dag_builder
            .layers(1..=4) // 4 rounds
            .authorities(vec![
                AuthorityIndex::new_for_test(0),
                AuthorityIndex::new_for_test(2),
            ]) // Create equivocating blocks for 2 authorities
            .equivocate(3) // Use 3 equivocations blocks per authority
            .build();

        // Take the blocks from round 4 up to 2 (included). Only the first block of each
        // round should return missing ancestors when try to accept
        for (_, block_header) in dag_builder
            .block_headers
            .into_iter()
            .rev()
            .take_while(|(_, block_header)| block_header.round() >= 2)
        {
            // WHEN
            let (accepted_blocks, missing) =
                block_manager.try_accept_block_headers(vec![block_header.clone()]);

            // THEN
            assert!(accepted_blocks.is_empty());

            let block_ancestors = block_header
                .ancestors()
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>();
            assert_eq!(missing, block_ancestors);
        }
    }

    #[tokio::test]
    async fn accept_blocks_with_complete_causal_history() {
        // GIVEN
        let (context, _key_pairs) = Context::new_for_test(4);
        let context = Arc::new(context);
        let store = Arc::new(MemStore::new());
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store.clone())));

        let mut block_manager = BlockManager::new(context.clone(), dag_state);

        // create a DAG of 2 rounds
        let mut dag_builder = DagBuilder::new(context.clone());
        dag_builder.layers(1..=2).build();

        let all_block_headers = dag_builder
            .block_headers
            .values()
            .cloned()
            .collect::<Vec<_>>();

        // WHEN
        let (accepted_block_headers, missing) =
            block_manager.try_accept_block_headers(all_block_headers.clone());

        // THEN
        assert_eq!(accepted_block_headers.len(), 8);
        assert_eq!(
            accepted_block_headers,
            all_block_headers
                .iter()
                .filter(|block_header| block_header.round() > 0)
                .cloned()
                .collect::<Vec<VerifiedBlockHeader>>()
        );
        assert!(missing.is_empty());
        assert!(block_manager.is_empty());

        // WHEN trying to accept same block headers again, then none will be returned as
        // those have been already accepted
        let (accepted_block_headers, _) = block_manager.try_accept_block_headers(all_block_headers);
        assert!(accepted_block_headers.is_empty());
    }

    /// The test generate blocks for a well connected DAG and feed them to block
    /// manager in random order. In the end all the blocks should be
    /// uniquely suspended and no missing blocks should exist.
    #[tokio::test]
    async fn accept_blocks_unsuspend_children_blocks() {
        // GIVEN
        let (context, _key_pairs) = Context::new_for_test(4);
        let context = Arc::new(context);

        // create a DAG of rounds 1 ~ 3
        let mut dag_builder = DagBuilder::new(context.clone());
        dag_builder.layers(1..=3).build();

        let mut all_block_headers = dag_builder
            .block_headers
            .values()
            .cloned()
            .collect::<Vec<_>>();

        // Now randomize the sequence of sending the blocks to block manager. In the end
        // all the blocks should be uniquely suspended and no missing blocks
        // should exist.
        for seed in 0..100u8 {
            all_block_headers.shuffle(&mut StdRng::from_seed([seed; 32]));

            let store = Arc::new(MemStore::new());
            let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store.clone())));

            let mut block_manager = BlockManager::new(context.clone(), dag_state);

            // WHEN
            let mut all_accepted_block_headers = vec![];
            for block_header in &all_block_headers {
                let (accepted_block_headers, _) =
                    block_manager.try_accept_block_headers(vec![block_header.clone()]);

                all_accepted_block_headers.extend(accepted_block_headers);
            }

            // THEN
            all_accepted_block_headers.sort_by_key(|b| b.reference());
            all_block_headers.sort_by_key(|b| b.reference());

            assert_eq!(
                all_accepted_block_headers, all_block_headers,
                "Failed acceptance sequence for seed {seed}",
            );
            assert!(block_manager.is_empty());
        }
    }

    /// Tests that `missing_blocks()` correctly infers the authorities
    /// referencing each missing block based on accepted blocks in the DAG.
    #[tokio::test]
    async fn authorities_that_know_missing_blocks() {
        let (context, _key_pairs) = Context::new_for_test(4);

        let context = Arc::new(context);

        // create a DAG of rounds 1 ~ 3
        let mut dag_builder = DagBuilder::new(context.clone());
        dag_builder.layers(1..=3).build();

        let all_blocks = dag_builder
            .block_headers
            .values()
            .cloned()
            .collect::<Vec<_>>();

        let blocks_round_2 = all_blocks
            .iter()
            .filter(|block| block.round() == 2)
            .cloned()
            .collect::<Vec<_>>();

        let blocks_round_1 = all_blocks
            .iter()
            .filter(|block| block.round() == 1)
            .map(|block| block.reference())
            .collect::<BTreeSet<_>>();

        let store = Arc::new(MemStore::new());
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store.clone())));

        let mut block_manager = BlockManager::new(context.clone(), dag_state);

        let (_, missing_blocks) =
            block_manager.try_accept_block_headers(vec![blocks_round_2[0].clone()]);
        // Blocks from round 1 are all missing, since the DAG is fully connected
        assert_eq!(missing_blocks, blocks_round_1);

        let missing_blocks_with_authorities = block_manager.missing_block_headers();

        let block_round_1_authority_0 = all_blocks
            .iter()
            .filter(|block| block.round() == 1 && block.author() == AuthorityIndex::new_for_test(0))
            .map(|block| block.reference())
            .next()
            .unwrap();
        let block_round_1_authority_1 = all_blocks
            .iter()
            .filter(|block| block.round() == 1 && block.author() == AuthorityIndex::new_for_test(1))
            .map(|block| block.reference())
            .next()
            .unwrap();
        assert_eq!(
            missing_blocks_with_authorities[&block_round_1_authority_0],
            BTreeSet::from([AuthorityIndex::new_for_test(0)])
        );
        assert_eq!(
            missing_blocks_with_authorities[&block_round_1_authority_1],
            BTreeSet::from([
                AuthorityIndex::new_for_test(0),
                AuthorityIndex::new_for_test(1)
            ])
        );

        // Add a new block from round 2 from authority 1, which updates the set of
        // authorities that are aware of the missing blocks
        block_manager.try_accept_block_headers(vec![blocks_round_2[1].clone()]);
        let missing_blocks_with_authorities = block_manager.missing_block_headers();
        assert_eq!(
            missing_blocks_with_authorities[&block_round_1_authority_0],
            BTreeSet::from([
                AuthorityIndex::new_for_test(0),
                AuthorityIndex::new_for_test(1)
            ])
        );
    }

    #[tokio::test]
    async fn reject_blocks_failing_verifications() {
        let (context, _key_pairs) = Context::new_for_test(4);
        let context = Arc::new(context);

        // create a DAG of rounds 1 ~ 5.
        let mut dag_builder = DagBuilder::new(context.clone());
        dag_builder.layer(1).build();
        // trigger failed verification by setting a timestamp delay
        // on layer 2 which are ancestors to round 3.
        dag_builder
            .layer(2)
            .configure_timestamp_delay_ms(5000)
            .build();
        dag_builder.layers(3..=5).build();

        let all_block_headers = dag_builder
            .block_headers
            .values()
            .cloned()
            .collect::<Vec<_>>();

        // Create BlockManager.
        let store = Arc::new(MemStore::new());
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store.clone())));
        let mut block_manager = BlockManager::new(context.clone(), dag_state);

        // Try to accept blocks from round 2 ~ 5 into block manager. All of them should
        // be suspended.
        let (accepted_block_headers, missing_refs) = block_manager.try_accept_block_headers(
            all_block_headers
                .iter()
                .filter(|block_header| block_header.round() > 1)
                .cloned()
                .collect(),
        );

        // Missing refs should all come from round 1.
        assert!(accepted_block_headers.is_empty());
        assert_eq!(missing_refs.len(), 4);
        missing_refs.iter().for_each(|missing_ref| {
            assert_eq!(missing_ref.round, 1);
        });

        // Now add round 1 blocks into block manager.
        let (accepted_block_headers, missing_refs) = block_manager.try_accept_block_headers(
            all_block_headers
                .iter()
                .filter(|block_header| block_header.round() == 1)
                .cloned()
                .collect(),
        );

        // Only round 1 and round 2 blocks should be accepted.
        assert_eq!(accepted_block_headers.len(), 8);
        accepted_block_headers.iter().for_each(|block_header| {
            assert!(block_header.round() <= 2);
        });
        assert!(missing_refs.is_empty());

        // Other blocks should be rejected and there should be no remaining suspended
        // block.
        assert!(block_manager.suspended_blocks_refs().is_empty());
    }

    #[tokio::test]
    async fn try_find_blocks() {
        // GIVEN
        let (context, _key_pairs) = Context::new_for_test(4);
        let context = Arc::new(context);
        let store = Arc::new(MemStore::new());
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store.clone())));

        let mut block_manager = BlockManager::new(context.clone(), dag_state);

        // create a DAG
        let mut dag_builder = DagBuilder::new(context.clone());
        dag_builder
            .layers(1..=2) // 2 rounds
            .authorities(vec![
                AuthorityIndex::new_for_test(0),
                AuthorityIndex::new_for_test(2),
            ]) // Create equivocating blocks for 2 authorities
            .equivocate(3)
            .build();

        // Take only the blocks of round 2 and try to accept them
        let round_2_block_headers = dag_builder
            .block_headers
            .iter()
            .filter_map(|(_, block_headers)| {
                (block_headers.round() == 2).then_some(block_headers.clone())
            })
            .collect::<Vec<VerifiedBlockHeader>>();

        // All blocks should be missing
        let missing_block_refs_from_find = block_manager.try_find_blocks(
            round_2_block_headers
                .iter()
                .map(|b| b.reference())
                .collect(),
        );
        assert_eq!(missing_block_refs_from_find.len(), 10);
        assert!(
            missing_block_refs_from_find
                .iter()
                .all(|block_ref| block_ref.round == 2)
        );

        // Try accept blocks which will cause blocks to be suspended and added to
        // missing in block manager.
        let (accepted_blocks_headers, missing) =
            block_manager.try_accept_block_headers(round_2_block_headers.clone());
        assert!(accepted_blocks_headers.is_empty());

        let missing_block_refs = round_2_block_headers.first().unwrap().ancestors();
        let missing_block_refs_from_accept =
            missing_block_refs.iter().cloned().collect::<BTreeSet<_>>();
        assert_eq!(missing, missing_block_refs_from_accept);
        assert_eq!(
            block_manager.missing_block_refs(),
            missing_block_refs_from_accept
        );

        // No blocks should be accepted and block manager should have made note
        // of the missing & suspended blocks.
        // Now we can check get the result of try find block with all of the blocks
        // from newly created but not accepted round 3.
        dag_builder.layer(3).build();

        let round_3_block_headers = dag_builder
            .block_headers
            .iter()
            .filter_map(|(_, block_header)| {
                (block_header.round() == 3).then_some(block_header.reference())
            })
            .collect::<Vec<BlockRef>>();

        let missing_block_refs_from_find = block_manager.try_find_blocks(
            round_2_block_headers
                .iter()
                .map(|b| b.reference())
                .chain(round_3_block_headers.into_iter())
                .collect(),
        );

        assert_eq!(missing_block_refs_from_find.len(), 4);
        assert!(
            missing_block_refs_from_find
                .iter()
                .all(|block_ref| block_ref.round == 3)
        );
        assert_eq!(
            block_manager.missing_block_refs(),
            missing_block_refs_from_accept
                .into_iter()
                .chain(missing_block_refs_from_find.into_iter())
                .collect()
        );
    }

    #[tokio::test]
    async fn test_check_ancestors() {
        let num_authorities = 4;
        let (context, _keypairs) = Context::new_for_test(num_authorities);
        let context = Arc::new(context);
        let store = Arc::new(MemStore::new());
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store.clone())));

        let block_manager = BlockManager::new(context.clone(), dag_state);

        let mut ancestor_blocks = vec![];
        for i in 0..num_authorities {
            let test_block = TestBlockHeader::new(10, i as u8)
                .set_timestamp_ms(1000 + 100 * i as BlockTimestampMs)
                .build();
            ancestor_blocks.push(VerifiedBlockHeader::new_for_test(test_block));
        }
        let ancestor_refs = ancestor_blocks
            .iter()
            .map(|block| block.reference())
            .collect::<Vec<_>>();

        // Block respecting timestamp invariant.
        {
            let block = TestBlockHeader::new(11, 0)
                .set_ancestors(ancestor_refs.clone())
                .set_timestamp_ms(1500)
                .build();
            let verified_block = VerifiedBlockHeader::new_for_test(block);
            assert!(
                block_manager
                    .check_ancestors(&verified_block, &ancestor_blocks)
                    .is_ok()
            );
        }

        // Block not respecting timestamp invariant.
        {
            let block = TestBlockHeader::new(11, 0)
                .set_ancestors(ancestor_refs.clone())
                .set_timestamp_ms(1000)
                .build();
            let verified_block = VerifiedBlockHeader::new_for_test(block);
            assert!(matches!(
                block_manager.check_ancestors(&verified_block, &ancestor_blocks,),
                Err(ConsensusError::InvalidBlockTimestamp {
                    max_timestamp_ms: _,
                    block_timestamp_ms: _
                })
            ));
        }
    }
}
