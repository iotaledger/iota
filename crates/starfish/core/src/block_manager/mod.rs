// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use iota_metrics::monitored_scope;
#[cfg(test)]
use itertools::Itertools as _;
use parking_lot::RwLock;
use starfish_config::AuthorityIndex;
#[cfg(test)]
use tracing::debug;
use tracing::warn;

/// Block Suspender is a private module unless under test.
#[cfg(not(test))]
mod block_suspender;
#[cfg(test)]
pub(crate) mod block_suspender;

use crate::{
    Round,
    block_header::{
        BlockHeaderAPI, BlockRef, VerifiedBlock, VerifiedBlockHeader, VerifiedTransactions,
    },
    block_manager::block_suspender::BlockSuspender,
    context::Context,
    dag_state::{DagState, DataSource},
};

/// Block manager suspends incoming blocks until they are connected to the
/// existing graph, returning newly connected blocks.
/// TODO: As it is possible to have Byzantine validators who produce Blocks
/// without valid causal history we need to make sure that BlockManager takes
/// care of that and avoid OOM (Out Of Memory) situations.
pub(crate) struct BlockManager {
    context: Arc<Context>,
    dag_state: Arc<RwLock<DagState>>,

    /// Keeps VerifiedTransactions of blocks whose headers have been suspended.
    /// TODO: this set can grow to become too big, need to add some eviction
    suspended_transactions: BTreeMap<BlockRef, VerifiedTransactions>,
    block_suspender: BlockSuspender,
    /// A vector that holds a tuple of (lowest_round, highest_round) of received
    /// blocks per authority. This is used for metrics reporting purposes
    /// and resets during restarts.
    received_block_rounds: Vec<Option<(Round, Round)>>,
}

impl BlockManager {
    pub(crate) fn new(context: Arc<Context>, dag_state: Arc<RwLock<DagState>>) -> Self {
        Self {
            context: context.clone(),
            dag_state,
            suspended_transactions: BTreeMap::new(),
            block_suspender: BlockSuspender::new(context.clone()),
            received_block_rounds: vec![None; context.committee.size()],
        }
    }

    /// Reinitialize BlockManager after fast sync completes.
    /// Clears suspended blocks and resets the block suspender.
    pub(crate) fn reinitialize(&mut self) {
        self.suspended_transactions.clear();
        self.block_suspender.reinitialize();
        self.received_block_rounds = vec![None; self.context.committee.size()];
    }

    /// Does all the same things as try_accept_block_headers and additionally
    /// saves blocks with transaction data into recent_blocks in DagState
    #[tracing::instrument(skip_all)]
    pub(crate) fn try_accept_blocks(
        &mut self,
        blocks: Vec<VerifiedBlock>,
        source: DataSource,
    ) -> (Vec<VerifiedBlockHeader>, BTreeSet<BlockRef>) {
        let _s = monitored_scope("BlockManager::try_accept_blocks");
        let block_headers: Vec<_> = blocks
            .iter()
            .map(|b| b.verified_block_header.clone())
            .collect();
        let present_header_and_ancestor_refs_in_dag_state =
            self.present_header_and_ancestor_refs_in_dag_state(&block_headers);
        let (block_headers_to_accept, missing_block_headers) = self.process_block_headers(
            block_headers,
            &present_header_and_ancestor_refs_in_dag_state,
            source,
        );
        // collect suspended transactions for accepted headers.
        let accepted_transactions = self.resolve_transactions(
            &block_headers_to_accept,
            &present_header_and_ancestor_refs_in_dag_state,
            Some(blocks),
        );

        self.write_block_headers_and_transactions_to_dag_state(
            block_headers_to_accept.clone(),
            accepted_transactions,
            source,
        );

        (block_headers_to_accept, missing_block_headers)
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
        source: DataSource,
    ) -> (Vec<VerifiedBlockHeader>, BTreeSet<BlockRef>) {
        let _s = monitored_scope("BlockManager::try_accept_block_headers");
        // Headers are added through synchronizer, commit syncer and cordial
        // dissemination.
        let present_header_and_ancestor_refs_in_dag_state =
            self.present_header_and_ancestor_refs_in_dag_state(&block_headers);
        let (block_headers_to_accept, ancestors_to_fetch) = self.process_block_headers(
            block_headers,
            &present_header_and_ancestor_refs_in_dag_state,
            source,
        );
        // collect transactions we already have for accepted headers.
        let accepted_transactions = self.resolve_transactions(
            &block_headers_to_accept,
            &present_header_and_ancestor_refs_in_dag_state,
            None,
        );
        self.write_block_headers_and_transactions_to_dag_state(
            block_headers_to_accept.clone(),
            accepted_transactions,
            source,
        );
        (block_headers_to_accept, ancestors_to_fetch)
    }

    /// Processes received block headers to determine which should be accepted,
    /// suspended, or fetched, and returns the accepted headers and missing
    /// ancestors
    fn process_block_headers(
        &mut self,
        block_headers: Vec<VerifiedBlockHeader>,
        present_header_and_ancestor_refs_in_dag_state: &BTreeSet<BlockRef>,
        source: DataSource,
    ) -> (Vec<VerifiedBlockHeader>, BTreeSet<BlockRef>) {
        let _s = monitored_scope("BlockManager::try_accept_block_headers_internal");

        // Filter out already processed and suspended block headers.
        let block_headers = self.filter_out_already_processed_and_sort(
            block_headers,
            present_header_and_ancestor_refs_in_dag_state,
            source,
        );
        // update received block rounds
        for block_header in &block_headers {
            self.update_block_received_metrics(block_header);
        }
        // Find missing ancestors for the provided block headers in the DAG state.
        let missing_ancestors = self
            .find_missing_ancestors(block_headers, present_header_and_ancestor_refs_in_dag_state);
        let (accepted_headers, missing_ancestors) = self
            .block_suspender
            .accept_or_suspend_received_headers(missing_ancestors);
        (accepted_headers, missing_ancestors)
    }

    fn write_block_headers_and_transactions_to_dag_state(
        &self,
        block_headers: Vec<VerifiedBlockHeader>,
        transactions: Vec<VerifiedTransactions>,
        source: DataSource,
    ) {
        let mut write_guard = self.dag_state.write();
        write_guard.accept_block_headers(block_headers, source);
        for verified_transaction in transactions {
            write_guard.add_transactions(verified_transaction, source);
        }
    }

    /// Resolves transactions from the provided blocks and accepted block
    /// headers.
    ///
    /// Moves transactions from suspended blocks whose headers are now accepted,
    /// and optionally processes newly received blocks, adding their
    /// transactions if accepted or re-suspending them otherwise.
    fn resolve_transactions(
        &mut self,
        block_headers_to_be_accepted: &[VerifiedBlockHeader],
        present_headers_and_ancestor_refs_in_dag_state: &BTreeSet<BlockRef>,
        blocks: Option<Vec<VerifiedBlock>>,
    ) -> Vec<VerifiedTransactions> {
        let block_refs_to_be_accepted = block_headers_to_be_accepted
            .iter()
            .map(|h| h.reference())
            .collect::<BTreeSet<_>>();
        let mut all_accepted_transactions = vec![];
        for block_ref in block_refs_to_be_accepted.iter() {
            if let Some(transactions) = self.suspended_transactions.remove(block_ref) {
                // for this accepted header we already have a block, so we add it to
                // accepted transactions
                all_accepted_transactions.push(transactions);
            }
        }

        if let Some(blocks) = blocks {
            let mut accepted_transactions_from_blocks = vec![];
            for block in blocks {
                if block_refs_to_be_accepted.contains(&block.reference())
                    || present_headers_and_ancestor_refs_in_dag_state.contains(&block.reference())
                {
                    accepted_transactions_from_blocks.push(block.verified_transactions);
                } else if block.verified_transactions.has_transactions() {
                    // optimization to avoid suspending 0 set verified transactions.
                    self.suspended_transactions
                        .insert(block.reference(), block.verified_transactions);
                }
            }
            all_accepted_transactions.extend(accepted_transactions_from_blocks);
        }
        self.context
            .metrics
            .node_metrics
            .block_manager_suspended_blocks
            .set(self.suspended_transactions.len() as i64);
        all_accepted_transactions
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

        let mut blocks_to_fetch = BTreeSet::new();

        for (found, block_ref) in self
            .dag_state
            .read()
            .contains_block_headers(block_refs.clone())
            .into_iter()
            .zip(block_refs.iter())
        {
            if found || self.block_suspender.is_block_ref_suspended(block_ref) {
                continue;
            }
            // Fetches the block if it is not in dag state or suspended.
            blocks_to_fetch.insert(*block_ref);
            if self
                .block_suspender
                .insert_block_to_fetch(*block_ref, BTreeSet::from([block_ref.author]))
                .is_none()
            {
                // We want to report this as a missing ancestor even if there is no block that
                // is actually references it right now.
                self.block_suspender
                    .set_missing_ancestors_with_no_children(*block_ref);

                self.context
                    .metrics
                    .node_metrics
                    .block_manager_missing_block_headers_by_authority
                    .with_label_values(&[self.context.authority_hostname(block_ref.author)])
                    .inc();
            }
        }

        let metrics = &self.context.metrics.node_metrics;
        metrics
            .missing_block_headers_total
            .inc_by(blocks_to_fetch.len() as u64);
        metrics
            .block_manager_missing_block_headers
            .set(self.block_suspender.blocks_to_fetch_len() as i64);

        blocks_to_fetch
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

    /// Returns all the blocks that are currently missing and needed in order to
    /// accept suspended blocks. For each block reference it returns the set of
    /// authorities who have this block.
    pub(crate) fn blocks_to_fetch(&self) -> BTreeMap<BlockRef, BTreeSet<AuthorityIndex>> {
        self.block_suspender.headers_to_fetch()
    }

    /// Returns all the block refs that are currently missing.
    #[cfg(test)]
    pub(crate) fn blocks_to_fetch_refs(&self) -> BTreeSet<BlockRef> {
        self.block_suspender.blocks_to_fetch_refs()
    }
    /// Checks if block manager is empty.
    #[cfg(test)]
    pub(crate) fn is_empty(&self) -> bool {
        self.block_suspender.is_empty()
    }

    /// Returns all the suspended blocks refs whose causal history we miss hence
    /// we can't accept them yet.
    #[cfg(test)]
    pub(crate) fn suspended_blocks_refs(&self) -> BTreeSet<BlockRef> {
        self.block_suspender.suspended_blocks_refs()
    }

    /// Returns the number of full blocks currently in suspended_blocks
    #[cfg(test)]
    pub(crate) fn suspended_full_blocks_count(&self) -> usize {
        self.suspended_transactions.len()
    }
    // helper method, to read the dag state once and output all present headers and
    // ancestors.
    fn present_header_and_ancestor_refs_in_dag_state(
        &self,
        block_headers: &[VerifiedBlockHeader],
    ) -> BTreeSet<BlockRef> {
        // make a single vector of references that contains both headers and ancestors
        // to check.
        let mut block_refs_and_ancestors = Vec::new();
        for h in block_headers {
            block_refs_and_ancestors.push(h.reference());
            block_refs_and_ancestors.extend(h.ancestors().iter().copied());
        }
        // deduplicate
        block_refs_and_ancestors.sort();
        block_refs_and_ancestors.dedup();
        // single dag_state read call
        let present_flags = self
            .dag_state
            .read()
            .contains_block_headers(block_refs_and_ancestors.clone());

        block_refs_and_ancestors
            .into_iter()
            .zip(present_flags)
            .filter_map(|(block_ref, found)| found.then_some(block_ref))
            .collect()
    }

    fn find_missing_ancestors(
        &self,
        incoming_headers: Vec<VerifiedBlockHeader>,
        present_header_and_ancestor_refs_in_dag_state: &BTreeSet<BlockRef>,
    ) -> BTreeMap<VerifiedBlockHeader, BTreeSet<BlockRef>> {
        let mut missing_ancestors = BTreeMap::new();
        for incoming_header in incoming_headers {
            let ancestors: &[BlockRef] = incoming_header.ancestors();
            let mut missing_ancestors_set = BTreeSet::new();
            for ancestor in ancestors {
                let found = present_header_and_ancestor_refs_in_dag_state.contains(ancestor);
                if !found {
                    missing_ancestors_set.insert(*ancestor);
                }
            }
            missing_ancestors.insert(incoming_header, missing_ancestors_set);
        }
        missing_ancestors
    }
    /// Filters out the block headers that have been already processed
    /// or are currently suspended. Reports metrics for the filtered out headers
    fn filter_out_already_processed_and_sort(
        &self,
        block_headers: Vec<VerifiedBlockHeader>,
        present_header_and_ancestor_refs_in_dag_state: &BTreeSet<BlockRef>,
        source: DataSource,
    ) -> Vec<VerifiedBlockHeader> {
        let mut filtered = block_headers
            .into_iter()
            .filter_map(|block_header| {
                let found = present_header_and_ancestor_refs_in_dag_state
                    .contains(&block_header.reference());
                if found
                    || self
                        .block_suspender
                        .is_block_ref_suspended(&block_header.reference())
                {
                    self.context
                        .metrics
                        .node_metrics
                        .core_skipped_headers
                        .with_label_values(&[
                            self.context.authority_hostname(block_header.author()),
                            source.as_str(),
                        ])
                        .inc();
                    None // filter out
                } else {
                    Some(block_header) // keep
                }
            })
            .collect::<Vec<_>>();
        filtered.sort_by_key(|h| h.round());
        filtered
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeSet, sync::Arc};

    use parking_lot::RwLock;
    use rand::{SeedableRng, prelude::StdRng, seq::SliceRandom};
    use starfish_config::AuthorityIndex;

    use crate::{
        block_header::{BlockHeaderAPI, BlockRef, VerifiedBlockHeader},
        block_manager::BlockManager,
        context::Context,
        dag_state::{DagState, DataSource},
        storage::mem_store::MemStore,
        test_dag_builder::DagBuilder,
        transaction_ref::GenericTransactionRef,
    };

    #[tokio::test]
    async fn suspend_blocks_with_missing_ancestors() {
        // GIVEN
        let (context, _key_pairs) = Context::new_for_test(4);
        let context = Arc::new(context);
        let store = Arc::new(MemStore::new(context.clone()));
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store)));

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
            block_manager.try_accept_block_headers(round_2_block_headers.clone(), DataSource::Test);

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
        assert_eq!(block_manager.blocks_to_fetch_refs(), missing_block_refs);

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
            .blocks_to_fetch()
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
        let store = Arc::new(MemStore::new(context.clone()));
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store)));

        let mut block_manager = BlockManager::new(context.clone(), dag_state);

        // create a DAG
        let mut dag_builder = DagBuilder::new(context);
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
            let (accepted_blocks, missing) = block_manager
                .try_accept_block_headers(vec![block_header.clone()], DataSource::Test);

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
        let store = Arc::new(MemStore::new(context.clone()));
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store)));

        let mut block_manager = BlockManager::new(context.clone(), dag_state);

        // create a DAG of 2 rounds
        let mut dag_builder = DagBuilder::new(context);
        dag_builder.layers(1..=2).build();

        let all_block_headers = dag_builder
            .block_headers
            .values()
            .cloned()
            .collect::<Vec<_>>();

        // WHEN
        let (accepted_block_headers, missing) =
            block_manager.try_accept_block_headers(all_block_headers.clone(), DataSource::Test);

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
        let (accepted_block_headers, _) =
            block_manager.try_accept_block_headers(all_block_headers, DataSource::Test);
        assert!(accepted_block_headers.is_empty());
    }

    /// The test generate blocks for a well-connected DAG and feed them to block
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

            let store = Arc::new(MemStore::new(context.clone()));
            let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store.clone())));

            let mut block_manager = BlockManager::new(context.clone(), dag_state);

            // WHEN
            let mut all_accepted_block_headers = vec![];
            for block_header in &all_block_headers {
                let (accepted_block_headers, _) = block_manager
                    .try_accept_block_headers(vec![block_header.clone()], DataSource::Test);

                all_accepted_block_headers.extend(accepted_block_headers);
            }

            // THEN
            all_accepted_block_headers.sort_by_key(|b| b.reference());
            all_block_headers.sort_by_key(|b| b.reference());

            assert_eq!(
                all_accepted_block_headers, all_block_headers,
                "Failed acceptance sequence for seed {seed}"
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

        let store = Arc::new(MemStore::new(context.clone()));
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store)));

        let mut block_manager = BlockManager::new(context, dag_state);

        let (_, missing_blocks) = block_manager
            .try_accept_block_headers(vec![blocks_round_2[0].clone()], DataSource::Test);
        // Blocks from round 1 are all missing, since the DAG is fully connected
        assert_eq!(missing_blocks, blocks_round_1);

        let missing_blocks_with_authorities = block_manager.blocks_to_fetch();

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
        block_manager.try_accept_block_headers(vec![blocks_round_2[1].clone()], DataSource::Test);
        let missing_blocks_with_authorities = block_manager.blocks_to_fetch();
        assert_eq!(
            missing_blocks_with_authorities[&block_round_1_authority_0],
            BTreeSet::from([
                AuthorityIndex::new_for_test(0),
                AuthorityIndex::new_for_test(1)
            ])
        );
    }

    #[tokio::test]
    async fn accept_blocks_with_timestamp_variations() {
        let (context, _key_pairs) = Context::new_for_test(4);
        let context = Arc::new(context);

        // create a DAG of rounds 1 ~ 5.
        let mut dag_builder = DagBuilder::new(context.clone());
        dag_builder.layer(1).build();
        // Set a timestamp delay on layer 2. With median-based timestamp,
        // blocks are no longer rejected for timestamp violations.
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
        let store = Arc::new(MemStore::new(context.clone()));
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store)));
        let mut block_manager = BlockManager::new(context, dag_state);
        // Try to accept blocks from round 2 ~ 5 into block manager. All of them should
        // be suspended.
        let (accepted_block_headers, missing_refs) = block_manager.try_accept_block_headers(
            all_block_headers
                .iter()
                .filter(|block_header| block_header.round() > 1)
                .cloned()
                .collect(),
            DataSource::Test,
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
            DataSource::Test,
        );
        // With median-based timestamp, all blocks should be accepted regardless of
        // timestamp violations.
        assert_eq!(accepted_block_headers.len(), 20); // 4 blocks * 5 rounds
        assert!(missing_refs.is_empty());
        assert!(block_manager.suspended_blocks_refs().is_empty());
    }

    #[tokio::test]
    async fn try_find_blocks() {
        // GIVEN
        let (context, _key_pairs) = Context::new_for_test(4);
        let context = Arc::new(context);
        let store = Arc::new(MemStore::new(context.clone()));
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store)));

        let mut block_manager = BlockManager::new(context.clone(), dag_state);

        // create a DAG
        let mut dag_builder = DagBuilder::new(context);
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

        // Try to accept blocks which will cause blocks to be suspended and added to
        // missing in block manager.
        let (accepted_blocks_headers, missing) =
            block_manager.try_accept_block_headers(round_2_block_headers.clone(), DataSource::Test);
        assert!(accepted_blocks_headers.is_empty());

        let missing_block_refs = round_2_block_headers.first().unwrap().ancestors();
        let missing_block_refs_from_accept =
            missing_block_refs.iter().cloned().collect::<BTreeSet<_>>();
        assert_eq!(missing, missing_block_refs_from_accept);
        assert_eq!(
            block_manager.blocks_to_fetch_refs(),
            missing_block_refs_from_accept
        );

        // No blocks should be accepted and block manager should have made note
        // of the missing & suspended blocks.
        // Now we can check get the result of try to find block with all the blocks
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
            block_manager.blocks_to_fetch_refs(),
            missing_block_refs_from_accept
                .into_iter()
                .chain(missing_block_refs_from_find.into_iter())
                .collect()
        );
    }

    /// Test that verifies the scenario where:
    /// 1. A header without transactions is added first and gets accepted
    /// 2. Later the full block with transactions is added
    /// 3. The bug: the full block gets stuck in suspended_blocks instead of
    ///    being processed
    ///
    /// Expected behavior:
    /// - When a full block arrives and its header is already accepted in
    ///   DagState, the transactions should be extracted and added to DagState
    /// - The full block should NOT remain in suspended_blocks
    ///
    /// Actual behavior (BUG):
    /// - The header is filtered out in filter_out_already_processed_and_sort()
    /// - block_headers_to_accept becomes empty
    /// - In resolve_transactions(), block_refs_to_be_accepted is empty
    /// - The full block gets added to suspended_blocks at line 186
    /// - Transactions are never added to DagState
    /// - The full block remains stuck in suspended_blocks forever
    #[tokio::test]
    async fn header_then_full_block_doesnt_leave_block_suspended() {
        // GIVEN
        let (context, _key_pairs) = Context::new_for_test(4);
        let context = Arc::new(context);
        let store = Arc::new(MemStore::new(context.clone()));
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store)));

        let mut block_manager = BlockManager::new(context.clone(), dag_state.clone());

        // Create a DAG with 2 rounds
        let mut dag_builder = DagBuilder::new(context.clone());
        dag_builder.layers(1..=2).build();

        let round_1_headers = dag_builder
            .block_headers
            .iter()
            .filter_map(|(_, block_header)| {
                (block_header.round() == 1).then_some(block_header.clone())
            })
            .collect::<Vec<_>>();

        // Get full blocks with transactions for round 2
        let round_2_blocks = dag_builder.blocks(2..=2);

        let round_2_headers = round_2_blocks
            .iter()
            .map(|b| b.verified_block_header.clone())
            .collect::<Vec<_>>();

        // WHEN: First, accept only the headers (without transactions) for round 1 and 2
        let (accepted_round_1_headers, missing) =
            block_manager.try_accept_block_headers(round_1_headers, DataSource::Test);
        assert_eq!(accepted_round_1_headers.len(), 4);
        assert!(missing.is_empty());

        let (accepted_round_2_headers, missing) =
            block_manager.try_accept_block_headers(round_2_headers.clone(), DataSource::Test);
        assert_eq!(accepted_round_2_headers.len(), 4);
        assert!(missing.is_empty());

        // Verify that the headers are now in DagState
        for header in &round_2_headers {
            assert!(dag_state.read().contains_block_header(&header.reference()));
        }

        // AND: Now try to accept the full blocks with transactions for round 2
        let (accepted_blocks, missing) =
            block_manager.try_accept_blocks(round_2_blocks.clone(), DataSource::Test);

        // THEN: The blocks should be accepted (headers already exist, just adding
        // transactions) But the suspected bug is that these blocks get stuck in
        // suspended_blocks
        assert_eq!(
            accepted_blocks.len(),
            0,
            "Expected headers to be returned as already processed"
        );
        assert!(missing.is_empty());

        // VERIFY: Check if the full blocks are stuck in suspended_blocks
        let suspended_count = block_manager.suspended_full_blocks_count();

        // Verify that transactions were actually added to DagState
        let has_transactions_results = dag_state.read().contains_transactions(
            round_2_blocks
                .iter()
                .map(|b| {
                    if context.protocol_config.consensus_fast_commit_sync() {
                        GenericTransactionRef::TransactionRef(b.transaction_ref())
                    } else {
                        GenericTransactionRef::BlockRef(b.reference())
                    }
                })
                .collect(),
        );

        let transactions_added_count = has_transactions_results.iter().filter(|&&x| x).count();

        // Print diagnostic information
        println!("Suspended full blocks count: {}", suspended_count);
        println!(
            "Transactions added to DagState: {}/{}",
            transactions_added_count,
            round_2_blocks.len()
        );

        // Assert the bug: suspended_blocks should be empty but it's not
        assert_eq!(
            suspended_count, 0,
            "BUG CONFIRMED: {} full blocks are stuck in suspended_blocks! They should have been processed or dropped.",
            suspended_count
        );

        // Assert that transactions should have been added
        for (block, has_transactions) in round_2_blocks.iter().zip(has_transactions_results.iter())
        {
            assert!(
                *has_transactions,
                "BUG CONFIRMED: Transactions should have been added to DagState for block {:?}",
                block.reference()
            );
        }
    }
}
