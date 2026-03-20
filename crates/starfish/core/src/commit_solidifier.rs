// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use parking_lot::RwLock;
use tracing::debug;

use crate::{
    CommitIndex, CommittedSubDag, commit::PendingSubDag, dag_state::DagState,
    transaction_ref::GenericTransactionRef,
};

/// The `CommitSolidifier` is responsible for managing and handling
/// the commit process for newly committed pending sub-dags. It ensures that
/// sub-dags that are sent for execution are solid, i.e. all transactions
/// included in the pending sub_dags are locally available and sub-dags are
/// committed in order. The `CommitSolidifier` also tracks the highest committed
/// index and maintains a buffer for pending sub-dags for which either the
/// transactions are not yet available or the previous sub-dags are missing
/// transactions and have not been output yet.
///
/// # Fields
/// - `dag_state`: Shared state of the DAG.
/// - `pending_subdags`: Buffer for sub-dags waiting to be committed.
/// - `last_committed_index`: Tracks the highest committed sub-dag index.
///
/// # Usage
/// The `CommitSolidifier` is used to process newly committed sub-dags by
/// retrieving information about potentially missing transactions.
pub(crate) struct CommitSolidifier {
    dag_state: Arc<RwLock<DagState>>,
    // Buffer for pending subdags, keyed by commit_ref.index for order
    pending_subdags: BTreeMap<u32, PendingSubDag>,
    // The highest solid commit_ref.index
    last_solid_committed_index: CommitIndex,
}

impl CommitSolidifier {
    pub(crate) fn new(dag_state: Arc<RwLock<DagState>>) -> Self {
        // the last_solid_committed_index is set non-trivially during a recovery process
        // before the first usage of try_commit method.
        let last_solid_committed_index = 0;
        Self {
            dag_state,
            pending_subdags: BTreeMap::new(),
            last_solid_committed_index,
        }
    }

    pub(crate) fn set_last_solid_committed_index(&mut self, index: CommitIndex) {
        self.last_solid_committed_index = index;
    }

    /// Gets all missing transactions from pending subdags.
    ///
    /// # Returns
    /// A `BTreeSet` of `GenericTransactionRef`s for which transactions are
    /// missing.
    pub(crate) fn get_missing_transaction_data(&self) -> BTreeSet<GenericTransactionRef> {
        let mut missing = BTreeSet::new();
        let dag_state = self.dag_state.read();

        // Check all pending subdags for missing transactions
        for subdag in self.pending_subdags.values() {
            let exists = dag_state.contains_transactions(subdag.committed_transaction_refs.clone());
            for (i, exists) in exists.iter().enumerate() {
                if !exists {
                    missing.insert(subdag.committed_transaction_refs[i]);
                }
            }
        }
        missing
    }

    /// Attempts to retrieve transactions included in the newly created commits.
    /// Adds the PendingSubDag to the buffer if any transactions are missing and
    /// outputs them once they are available.
    ///
    /// # Arguments
    /// - `subdags`: A slice of `PendingSubDag` to be committed.
    ///
    /// # Returns
    /// A tuple containing:
    /// - `Vec<CommittedSubDag>`: Successfully committed sub-dags.
    /// - `Vec<BlockRef>`: References to blocks with missing transactions
    ///   preventing further commits.
    pub(crate) fn try_get_solid_sub_dags(
        &mut self,
        subdags: &[PendingSubDag],
    ) -> (Vec<CommittedSubDag>, Vec<GenericTransactionRef>) {
        // Add new subdags to the buffer
        for subdag in subdags {
            self.pending_subdags
                .entry(subdag.commit_ref.index)
                .or_insert_with(|| subdag.clone());
        }
        let mut committed_subdags = Vec::new();
        let mut last_solid_committed = self.last_solid_committed_index;
        let mut missing = BTreeSet::new();

        // Try to commit in order
        loop {
            let next_index = last_solid_committed + 1;
            // If the next expected subdag is not in the buffer, we cannot commit anything
            // further
            let Some(pending_subdag) = self.pending_subdags.get(&next_index) else {
                break;
            };
            match self.try_get_one_solid_sub_dag_internal(pending_subdag) {
                Ok(committed_subdag) => {
                    committed_subdags.push(committed_subdag);
                    self.pending_subdags.remove(&next_index);
                    last_solid_committed = next_index;
                }
                Err(missing_refs) => {
                    // If we have missing refs, we cannot commit this subdag
                    debug!(
                        "Cannot create CommittedSubDag at index {}. Missing refs: {:?}",
                        next_index, missing_refs
                    );

                    break; // Can't commit further until this one is ready
                }
            }
        }

        // Update with the latest solid_commit_refs which are used for commit syncer
        if !committed_subdags.is_empty() {
            let solid_commit_refs = committed_subdags
                .iter()
                .map(|subdag| subdag.commit_ref)
                .collect();
            self.dag_state
                .write()
                .update_pending_commit_votes(solid_commit_refs);
        }

        // Update last_committed_index
        self.last_solid_committed_index = last_solid_committed;

        // Only check for missing refs in the newly passed subdags that weren't
        // processed yet
        for subdag in subdags {
            if subdag.commit_ref.index > self.last_solid_committed_index {
                // Query dag_state directly for missing transactions
                let dag_state_guard = self.dag_state.read();
                let exists = dag_state_guard
                    .contains_transactions(subdag.committed_transaction_refs.clone());
                drop(dag_state_guard);
                for (i, exists) in exists.iter().enumerate() {
                    if !exists {
                        let block_ref = subdag.committed_transaction_refs[i];
                        if !missing.insert(block_ref) {
                            // Transactions should only be committed by a single subdag, so
                            // duplicates should never happen.
                            panic!("Duplicate missing blockref detected: {block_ref:?}");
                        }
                    }
                }
            }
        }

        (committed_subdags, missing.into_iter().collect())
    }

    /// Internal method to retrieve all transactions committed to PendingSubDag
    /// and form CommittedSubDAG if all of them are locally available
    ///
    /// # Arguments
    /// - `subdag`: A reference to the `PendingSubDag` to be committed.
    ///
    /// # Returns
    /// - `Ok(CommittedSubDag)`: If all required transactions are in DagState
    /// - `Err(Vec<BlockRef>)`: If some transactions are missing.
    fn try_get_one_solid_sub_dag_internal(
        &self,
        subdag: &PendingSubDag,
    ) -> Result<CommittedSubDag, Vec<GenericTransactionRef>> {
        let dag_state = self.dag_state.read();
        // Get transactions and check if any are missing
        let transaction_results =
            dag_state.get_verified_transactions(&subdag.committed_transaction_refs);
        let mut missing = Vec::new();
        for (i, tx_opt) in transaction_results.iter().enumerate() {
            if tx_opt.is_none() {
                missing.push(subdag.committed_transaction_refs[i]);
            }
        }

        if missing.is_empty() {
            // All transactions exist, so we can create a CommittedSubDag
            let transactions = transaction_results
                .into_iter()
                .map(|tx| tx.expect("Transaction must exist since we checked"))
                .collect();

            Ok(CommittedSubDag::new(
                subdag.leader,
                subdag.base.headers.clone(),
                subdag.base.committed_header_refs.clone(),
                transactions,
                subdag.timestamp_ms,
                subdag.commit_ref,
                subdag.reputation_scores_desc.clone(),
            ))
        } else {
            Err(missing)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use parking_lot::RwLock;
    use rstest::rstest;

    use super::*;
    use crate::{
        block_header::{genesis_block_headers, genesis_blocks},
        commit::{CommitRef, PendingSubDag},
        context::Context,
        dag_state::{DagState, DataSource},
        storage::mem_store::MemStore,
        test_dag_builder::DagBuilder,
    };

    /// Test helper struct to encapsulate common test setup and utilities
    struct TestSetup {
        dag_state: Arc<RwLock<DagState>>,
        dag_builder: DagBuilder,
        context: Arc<Context>,
    }

    impl TestSetup {
        /// Creates a new test setup with a full DAG containing the specified
        /// number of rounds
        fn new(num_rounds: u32, consensus_fast_commit_sync: bool) -> Self {
            let (mut context, _) = Context::new_for_test(2);
            context
                .protocol_config
                .set_consensus_fast_commit_sync_for_testing(consensus_fast_commit_sync);
            context.parameters.enable_fast_commit_syncer = consensus_fast_commit_sync;
            let context = Arc::new(context);
            let dag_state = Arc::new(RwLock::new(DagState::new(
                context.clone(),
                Arc::new(crate::storage::mem_store::MemStore::new(context.clone())),
            )));
            let mut dag_builder = DagBuilder::new(context.clone());
            dag_builder
                .layers(1..=num_rounds)
                .build()
                .persist_layers(dag_state.clone());

            Self {
                dag_state,
                dag_builder,
                context,
            }
        }

        /// Creates a selective DAG state that only contains transactions from
        /// specified rounds
        ///
        /// # Arguments
        /// * `included_rounds` - Vector of round numbers whose transactions
        ///   should be included
        /// * `excluded_transactions` - Vector of (round, block_index) pairs to
        ///   exclude transactions from specific blocks
        fn create_selective_dag_state(
            &self,
            included_rounds: Vec<u32>,
            excluded_transactions: Vec<(u32, usize)>,
        ) -> Arc<RwLock<DagState>> {
            let selective_dag_state = Arc::new(RwLock::new(DagState::new(
                self.context.clone(),
                Arc::new(MemStore::new(self.context.clone())),
            )));

            let mut state = selective_dag_state.write();

            // Add genesis blocks if round 0 is included
            if included_rounds.contains(&0) {
                let genesis_blocks = genesis_blocks(&self.context);
                for (i, block) in genesis_blocks.iter().enumerate() {
                    state
                        .accept_block_header(block.verified_block_header.clone(), DataSource::Test);
                    if !excluded_transactions.contains(&(0, i)) {
                        state.add_transactions(
                            block.verified_transactions.clone(),
                            DataSource::Test,
                        );
                    }
                }
            }

            // Add blocks from specified rounds
            for &round in &included_rounds {
                if round == 0 {
                    continue;
                } // Genesis blocks already handled

                let blocks = self.dag_builder.blocks(round..=round);
                for (i, block) in blocks.iter().enumerate() {
                    state
                        .accept_block_header(block.verified_block_header.clone(), DataSource::Test);
                    if !excluded_transactions.contains(&(round, i)) {
                        state.add_transactions(
                            block.verified_transactions.clone(),
                            DataSource::Test,
                        );
                    }
                }
            }

            drop(state);
            selective_dag_state
        }

        /// Creates a CommitSolidifier with a selective DAG state
        fn create_selective_commit_solidifier(
            &self,
            included_rounds: Vec<u32>,
            excluded_blocks: Vec<(u32, usize)>,
        ) -> (CommitSolidifier, Arc<RwLock<DagState>>) {
            let selective_dag_state =
                self.create_selective_dag_state(included_rounds, excluded_blocks);
            let commit_solidifier = CommitSolidifier::new(selective_dag_state.clone());
            (commit_solidifier, selective_dag_state)
        }

        /// Adds missing transactions for specific blocks back to the DAG state
        fn add_missing_transactions(
            &self,
            dag_state: &Arc<RwLock<DagState>>,
            blocks: &[(u32, usize)],
        ) {
            let mut state = dag_state.write();
            for &(round, block_index) in blocks {
                if round == 0 {
                    let genesis_blocks = genesis_blocks(&self.context);
                    if let Some(block) = genesis_blocks.get(block_index) {
                        state.add_transactions(
                            block.verified_transactions.clone(),
                            DataSource::Test,
                        );
                    }
                } else {
                    let blocks = self.dag_builder.blocks(round..=round);
                    if let Some(block) = blocks.get(block_index) {
                        state.add_transactions(
                            block.verified_transactions.clone(),
                            DataSource::Test,
                        );
                    }
                }
            }
        }
    }

    /// Builder for creating PendingSubDag instances with a fluent API
    struct SubDagBuilder {
        index: u32,
        leader_round: u32,
        leader_index: usize,
        block_specs: Vec<BlockSpec>,
        committed_refs: Vec<GenericTransactionRef>,
        setup: Arc<TestSetup>,
    }

    #[derive(Clone)]
    struct BlockSpec {
        round: u32,
        indices: Option<Vec<usize>>, // None means all blocks, Some(vec) means specific indices
    }

    impl BlockSpec {
        fn all_from_round(round: u32) -> Self {
            Self {
                round,
                indices: None,
            }
        }

        fn specific_from_round(round: u32, indices: Vec<usize>) -> Self {
            Self {
                round,
                indices: Some(indices),
            }
        }

        fn skip_first_from_round(round: u32) -> Self {
            // Helper to skip the first block
            Self {
                round,
                indices: Some(vec![]),
            } // Will be populated dynamically
        }
    }

    impl SubDagBuilder {
        fn new(setup: Arc<TestSetup>, index: u32) -> Self {
            Self {
                index,
                leader_round: 0,
                leader_index: 0,
                block_specs: Vec::new(),
                committed_refs: Vec::new(),
                setup,
            }
        }

        fn leader(mut self, round: u32, index: usize) -> Self {
            self.leader_round = round;
            self.leader_index = index;
            self
        }

        fn with_blocks(mut self, specs: Vec<BlockSpec>) -> Self {
            self.block_specs = specs;
            self
        }

        fn with_committed_refs_from_round(mut self, round: u32) -> Self {
            let consensus_fast_commit_sync = self
                .setup
                .context
                .protocol_config
                .consensus_fast_commit_sync();
            let refs = if round == 0 {
                if consensus_fast_commit_sync {
                    genesis_blocks(&self.setup.context)
                        .iter()
                        .map(|b| {
                            GenericTransactionRef::TransactionRef(
                                b.verified_block_header.transaction_ref(),
                            )
                        })
                        .collect()
                } else {
                    genesis_blocks(&self.setup.context)
                        .iter()
                        .map(|b| GenericTransactionRef::from(b.reference()))
                        .collect()
                }
            } else if consensus_fast_commit_sync {
                self.setup
                    .dag_builder
                    .block_headers(round..=round)
                    .iter()
                    .map(|b| GenericTransactionRef::TransactionRef(b.transaction_ref()))
                    .collect()
            } else {
                self.setup
                    .dag_builder
                    .block_headers(round..=round)
                    .iter()
                    .map(|b| GenericTransactionRef::from(b.reference()))
                    .collect()
            };
            self.committed_refs = refs;
            self
        }

        fn with_committed_refs(mut self, refs: Vec<GenericTransactionRef>) -> Self {
            self.committed_refs = refs;
            self
        }

        fn build(self) -> PendingSubDag {
            // Get leader block
            let leader = if self.leader_round == 0 {
                genesis_blocks(&self.setup.context)[self.leader_index].reference()
            } else {
                self.setup
                    .dag_builder
                    .block_headers(self.leader_round..=self.leader_round)[self.leader_index]
                    .reference()
            };

            // Collect all blocks based on specs
            let mut all_committed_block_headers = Vec::new();

            for spec in &self.block_specs {
                let headers = if spec.round == 0 {
                    genesis_block_headers(&self.setup.context)
                } else {
                    self.setup
                        .dag_builder
                        .block_headers(spec.round..=spec.round)
                };

                match &spec.indices {
                    None => all_committed_block_headers.extend(headers),
                    Some(indices) => {
                        if indices.is_empty() {
                            // Special case: skip first
                            all_committed_block_headers.extend(headers.into_iter().skip(1));
                        } else {
                            for &i in indices {
                                if let Some(header) = headers.get(i) {
                                    all_committed_block_headers.push(header.clone());
                                }
                            }
                        }
                    }
                }
            }

            // Add a leader block if not already included
            let leader_header = if self.leader_round == 0 {
                genesis_blocks(&self.setup.context)[self.leader_index]
                    .verified_block_header
                    .clone()
            } else {
                self.setup
                    .dag_builder
                    .block_headers(self.leader_round..=self.leader_round)[self.leader_index]
                    .clone()
            };

            if !all_committed_block_headers
                .iter()
                .any(|b| b.reference() == leader)
            {
                all_committed_block_headers.push(leader_header);
            }

            PendingSubDag::new(
                leader,
                all_committed_block_headers.clone(),
                all_committed_block_headers
                    .iter()
                    .map(|h| h.reference())
                    .collect(),
                self.committed_refs,
                123456,
                CommitRef {
                    index: self.index,
                    digest: crate::commit::CommitDigest::MIN,
                },
                vec![],
            )
        }
    }

    /// Tests the happy path where a single sub-dag is successfully committed.
    #[rstest]
    #[tokio::test]
    async fn test_happy_path_commit(#[values(false, true)] consensus_fast_commit_sync: bool) {
        let setup = Arc::new(TestSetup::new(3, consensus_fast_commit_sync));
        let mut commit_solidifier = CommitSolidifier::new(setup.dag_state.clone());

        let subdag = SubDagBuilder::new(setup, 1)
            .leader(3, 0)
            .with_blocks(vec![
                BlockSpec::all_from_round(0),
                BlockSpec::all_from_round(1),
                BlockSpec::all_from_round(2),
            ])
            .with_committed_refs_from_round(1)
            .build();

        let (committed, missing) = commit_solidifier.try_get_solid_sub_dags(&[subdag]);
        assert_eq!(committed.len(), 1);
        assert!(missing.is_empty());
        assert_eq!(commit_solidifier.last_solid_committed_index, 1);
        assert!(commit_solidifier.pending_subdags.is_empty());
    }

    #[rstest]
    #[tokio::test]
    async fn test_missing_blocks(#[values(false, true)] consensus_fast_commit_sync: bool) {
        let setup = Arc::new(TestSetup::new(3, consensus_fast_commit_sync));
        let (mut commit_solidifier, _selective_dag_state) = setup
            .create_selective_commit_solidifier(
                vec![1, 2, 3],
                vec![(1, 0)], // Exclude the first transaction from round 1
            );

        let committed_ref = if consensus_fast_commit_sync {
            GenericTransactionRef::TransactionRef(
                setup.dag_builder.block_headers(1..=1)[0].transaction_ref(),
            )
        } else {
            GenericTransactionRef::from(setup.dag_builder.block_headers(1..=1)[0].reference())
        };

        let subdag = SubDagBuilder::new(setup, 1)
            .leader(3, 0)
            .with_blocks(vec![
                BlockSpec::all_from_round(0),
                BlockSpec::all_from_round(1),
                BlockSpec::all_from_round(2),
            ])
            .with_committed_refs(vec![committed_ref])
            .build();

        let (committed, missing) = commit_solidifier.try_get_solid_sub_dags(&[subdag]);
        assert!(committed.is_empty());
        assert_eq!(missing.len(), 1);
        assert_eq!(missing[0], committed_ref);
        assert_eq!(commit_solidifier.pending_subdags.len(), 1);
        assert_eq!(commit_solidifier.last_solid_committed_index, 0);
    }

    #[rstest]
    #[tokio::test]
    async fn test_commit_after_missing_blocks_arrive(
        #[values(false, true)] consensus_fast_commit_sync: bool,
    ) {
        let setup = Arc::new(TestSetup::new(3, consensus_fast_commit_sync));
        let (mut commit_solidifier, selective_dag_state) = setup
            .create_selective_commit_solidifier(
                vec![1, 2, 3],
                vec![(1, 0)], // Exclude the first transactions from round 1
            );

        let committed_ref = if consensus_fast_commit_sync {
            GenericTransactionRef::TransactionRef(
                setup.dag_builder.block_headers(1..=1)[0].transaction_ref(),
            )
        } else {
            GenericTransactionRef::from(setup.dag_builder.block_headers(1..=1)[0].reference())
        };

        let subdag = SubDagBuilder::new(setup.clone(), 1)
            .leader(3, 0)
            .with_blocks(vec![
                BlockSpec::all_from_round(0),
                BlockSpec::all_from_round(1),
                BlockSpec::all_from_round(2),
            ])
            .with_committed_refs(vec![committed_ref])
            .build();

        // The first attempt should fail due to a missing block
        let (committed, missing) =
            commit_solidifier.try_get_solid_sub_dags(std::slice::from_ref(&subdag));
        assert!(committed.is_empty());
        assert_eq!(missing.len(), 1);

        // Add the missing block
        setup.add_missing_transactions(&selective_dag_state, &[(1, 0)]);

        // The second attempt should succeed
        let (committed, missing) = commit_solidifier.try_get_solid_sub_dags(&[]);
        assert_eq!(committed.len(), 1);
        assert!(missing.is_empty());
        assert!(commit_solidifier.pending_subdags.is_empty());
        assert_eq!(commit_solidifier.last_solid_committed_index, 1);
    }

    #[rstest]
    #[tokio::test]
    async fn test_multiple_subdags_in_order(
        #[values(false, true)] consensus_fast_commit_sync: bool,
    ) {
        let setup = Arc::new(TestSetup::new(4, consensus_fast_commit_sync));
        let mut commit_solidifier = CommitSolidifier::new(setup.dag_state.clone());

        let subdag1 = SubDagBuilder::new(setup.clone(), 1)
            .leader(3, 0)
            .with_blocks(vec![
                BlockSpec::all_from_round(0),
                BlockSpec::all_from_round(1),
                BlockSpec::all_from_round(2),
            ])
            .with_committed_refs_from_round(1)
            .build();

        let subdag2 = SubDagBuilder::new(setup, 2)
            .leader(4, 0)
            .with_blocks(vec![BlockSpec::skip_first_from_round(3)])
            .with_committed_refs_from_round(2)
            .build();

        let (committed, missing) = commit_solidifier.try_get_solid_sub_dags(&[subdag1, subdag2]);
        assert_eq!(committed.len(), 2);
        assert!(missing.is_empty());
        assert!(commit_solidifier.pending_subdags.is_empty());
        assert_eq!(commit_solidifier.last_solid_committed_index, 2);
    }

    #[rstest]
    #[tokio::test]
    async fn test_out_of_order_subdags(#[values(false, true)] consensus_fast_commit_sync: bool) {
        let setup = Arc::new(TestSetup::new(4, consensus_fast_commit_sync));
        let mut commit_solidifier = CommitSolidifier::new(setup.dag_state.clone());

        let subdag1 = SubDagBuilder::new(setup.clone(), 1)
            .leader(3, 0)
            .with_blocks(vec![
                BlockSpec::all_from_round(0),
                BlockSpec::all_from_round(1),
                BlockSpec::all_from_round(2),
            ])
            .with_committed_refs_from_round(1)
            .build();

        let subdag2 = SubDagBuilder::new(setup, 2)
            .leader(4, 0)
            .with_blocks(vec![BlockSpec::skip_first_from_round(3)])
            .with_committed_refs_from_round(2)
            .build();

        // Submit out of order
        let (committed, missing) = commit_solidifier.try_get_solid_sub_dags(&[subdag2, subdag1]);
        assert_eq!(committed.len(), 2);
        assert!(missing.is_empty());
        assert!(commit_solidifier.pending_subdags.is_empty());
        assert_eq!(commit_solidifier.last_solid_committed_index, 2);

        // The second call should be no-op
        let (committed, missing) = commit_solidifier.try_get_solid_sub_dags(&[]);
        assert!(committed.is_empty());
        assert!(missing.is_empty());
        assert!(commit_solidifier.pending_subdags.is_empty());
        assert_eq!(commit_solidifier.last_solid_committed_index, 2);
    }

    #[rstest]
    #[tokio::test]
    async fn test_empty_subdag_commit(#[values(false, true)] consensus_fast_commit_sync: bool) {
        let setup = Arc::new(TestSetup::new(2, consensus_fast_commit_sync));
        let mut commit_solidifier = CommitSolidifier::new(setup.dag_state.clone());

        let (committed, missing) = commit_solidifier.try_get_solid_sub_dags(&[]);
        assert!(committed.is_empty());
        assert!(missing.is_empty());
        assert!(commit_solidifier.pending_subdags.is_empty());
        assert_eq!(commit_solidifier.last_solid_committed_index, 0);
    }

    #[rstest]
    #[tokio::test]
    async fn test_duplicate_subdag_commit(#[values(false, true)] consensus_fast_commit_sync: bool) {
        let setup = Arc::new(TestSetup::new(3, consensus_fast_commit_sync));
        let mut commit_solidifier = CommitSolidifier::new(setup.dag_state.clone());

        let subdag1 = SubDagBuilder::new(setup, 1)
            .leader(3, 0)
            .with_blocks(vec![
                BlockSpec::all_from_round(0),
                BlockSpec::all_from_round(1),
                BlockSpec::all_from_round(2),
            ])
            .with_committed_refs_from_round(1)
            .build();

        let (committed, missing) =
            commit_solidifier.try_get_solid_sub_dags(&[subdag1.clone(), subdag1]);
        assert_eq!(committed.len(), 1);
        assert!(missing.is_empty());
        assert!(commit_solidifier.pending_subdags.is_empty());
        assert_eq!(commit_solidifier.last_solid_committed_index, 1);
    }

    #[rstest]
    #[tokio::test]
    async fn test_out_of_order_commit_calls(
        #[values(false, true)] consensus_fast_commit_sync: bool,
    ) {
        let setup = Arc::new(TestSetup::new(4, consensus_fast_commit_sync));
        let mut commit_solidifier = CommitSolidifier::new(setup.dag_state.clone());

        let subdag1 = SubDagBuilder::new(setup.clone(), 1)
            .leader(3, 0)
            .with_blocks(vec![
                BlockSpec::all_from_round(0),
                BlockSpec::all_from_round(1),
                BlockSpec::all_from_round(2),
            ])
            .with_committed_refs_from_round(1)
            .build();

        let subdag2 = SubDagBuilder::new(setup, 2)
            .leader(4, 0)
            .with_blocks(vec![BlockSpec::skip_first_from_round(3)])
            .with_committed_refs_from_round(2)
            .build();

        // First submit subdag2 (index 2)
        let (committed, missing) =
            commit_solidifier.try_get_solid_sub_dags(std::slice::from_ref(&subdag2));
        assert!(committed.is_empty());
        assert!(missing.is_empty());
        assert!(commit_solidifier.pending_subdags.contains_key(&2));
        assert_eq!(commit_solidifier.last_solid_committed_index, 0);

        // Then submit subdag1 (index 1) - should commit both
        let (committed, missing) =
            commit_solidifier.try_get_solid_sub_dags(std::slice::from_ref(&subdag1));
        assert_eq!(committed.len(), 2);
        assert!(missing.is_empty());
        assert!(commit_solidifier.pending_subdags.is_empty());
        assert_eq!(commit_solidifier.last_solid_committed_index, 2);
    }

    #[rstest]
    #[tokio::test]
    async fn test_all_missing_refs_are_collected(
        #[values(false, true)] consensus_fast_commit_sync: bool,
    ) {
        telemetry_subscribers::init_for_testing();

        let setup = Arc::new(TestSetup::new(4, consensus_fast_commit_sync));
        let (mut commit_solidifier, selective_dag_state) = setup
            .create_selective_commit_solidifier(
                vec![1, 2, 3, 4],
                vec![(1, 0), (2, 0)], // Exclude the first transactions from rounds 1 and 2
            );

        let subdag1 = SubDagBuilder::new(setup.clone(), 1)
            .leader(2, 0)
            .with_blocks(vec![
                BlockSpec::all_from_round(0),
                BlockSpec::all_from_round(1),
            ])
            .with_committed_refs(vec![]) // No committed refs
            .build();

        let subdag2_committed_ref = if consensus_fast_commit_sync {
            GenericTransactionRef::TransactionRef(
                setup.dag_builder.block_headers(1..=1)[0].transaction_ref(),
            )
        } else {
            GenericTransactionRef::from(setup.dag_builder.block_headers(1..=1)[0].reference())
        };

        let subdag2 = SubDagBuilder::new(setup.clone(), 2)
            .leader(3, 0)
            .with_blocks(vec![BlockSpec::skip_first_from_round(2)])
            .with_committed_refs(vec![subdag2_committed_ref])
            .build();

        let subdag3_committed_ref = if consensus_fast_commit_sync {
            GenericTransactionRef::TransactionRef(
                setup.dag_builder.block_headers(2..=2)[0].transaction_ref(),
            )
        } else {
            GenericTransactionRef::from(setup.dag_builder.block_headers(2..=2)[0].reference())
        };

        let subdag3 = SubDagBuilder::new(setup.clone(), 3)
            .leader(4, 0)
            .with_blocks(vec![BlockSpec::skip_first_from_round(3)])
            .with_committed_refs(vec![subdag3_committed_ref])
            .build();

        // Initial commit attempts
        let (committed, missing) =
            commit_solidifier.try_get_solid_sub_dags(std::slice::from_ref(&subdag3));
        assert!(committed.is_empty());
        assert_eq!(missing.len(), 1);
        assert_eq!(commit_solidifier.pending_subdags.len(), 1);

        let (committed, missing) =
            commit_solidifier.try_get_solid_sub_dags(std::slice::from_ref(&subdag2));
        assert!(committed.is_empty());
        assert_eq!(missing.len(), 1);
        assert_eq!(commit_solidifier.pending_subdags.len(), 2);

        let (committed, missing) =
            commit_solidifier.try_get_solid_sub_dags(std::slice::from_ref(&subdag1));
        assert!(missing.is_empty());
        assert_eq!(committed.len(), 1); // subdag1 can commit
        assert_eq!(committed[0].commit_ref, subdag1.commit_ref);
        assert_eq!(commit_solidifier.pending_subdags.len(), 2);

        // Add missing block from round 1
        setup.add_missing_transactions(&selective_dag_state, &[(1, 0)]);
        let (committed, _missing) = commit_solidifier.try_get_solid_sub_dags(&[]);
        assert_eq!(committed.len(), 1); // subdag2 commits
        assert_eq!(committed[0].commit_ref, subdag2.commit_ref);
        assert_eq!(commit_solidifier.last_solid_committed_index, 2);

        // Add missing block from round 2
        setup.add_missing_transactions(&selective_dag_state, &[(2, 0)]);
        let (committed, _missing) = commit_solidifier.try_get_solid_sub_dags(&[]);
        assert_eq!(committed.len(), 1); // subdag3 commits
        assert_eq!(committed[0].commit_ref, subdag3.commit_ref);
        assert_eq!(commit_solidifier.last_solid_committed_index, 3);
        assert!(commit_solidifier.pending_subdags.is_empty());
    }

    #[rstest]
    #[tokio::test]
    #[should_panic(expected = "Duplicate missing blockref detected")]
    async fn test_duplicate_missing_refs_panic(
        #[values(false, true)] consensus_fast_commit_sync: bool,
    ) {
        let setup = Arc::new(TestSetup::new(4, consensus_fast_commit_sync));
        let (mut commit_solidifier, _selective_dag_state) = setup
            .create_selective_commit_solidifier(
                vec![1, 2, 3, 4],
                vec![(1, 0)], // Exclude the first transactions from round 1
            );

        let subdag1 = SubDagBuilder::new(setup.clone(), 1)
            .leader(2, 0)
            .with_blocks(vec![
                BlockSpec::all_from_round(0),
                BlockSpec::all_from_round(1),
            ])
            .with_committed_refs(vec![])
            .build();

        let subdag2_committed_ref = if consensus_fast_commit_sync {
            GenericTransactionRef::TransactionRef(
                setup.dag_builder.block_headers(1..=1)[0].transaction_ref(),
            )
        } else {
            GenericTransactionRef::from(setup.dag_builder.block_headers(1..=1)[0].reference())
        };

        let subdag2 = SubDagBuilder::new(setup.clone(), 2)
            .leader(3, 0)
            .with_blocks(vec![BlockSpec::skip_first_from_round(1)])
            .with_committed_refs(vec![subdag2_committed_ref])
            .build();

        let committed_refs_for_subdag3 = [
            &setup.dag_builder.block_headers(1..=1)[0],
            &setup.dag_builder.block_headers(2..=2)[0],
        ]
        .iter()
        .map(|header| {
            if consensus_fast_commit_sync {
                GenericTransactionRef::TransactionRef(header.transaction_ref())
            } else {
                GenericTransactionRef::from(header.reference())
            }
        })
        .collect::<Vec<_>>();

        let subdag3 = SubDagBuilder::new(setup, 2) // Same index as subdag2
            .leader(4, 0)
            .with_blocks(vec![BlockSpec::skip_first_from_round(3)])
            .with_committed_refs(committed_refs_for_subdag3)
            .build();

        // This should panic due to a duplicate missing block ref
        commit_solidifier.try_get_solid_sub_dags(&[subdag1, subdag2, subdag3]);
    }

    #[rstest]
    #[tokio::test]
    async fn test_gaps_in_subdags_sequence(
        #[values(false, true)] consensus_fast_commit_sync: bool,
    ) {
        let setup = Arc::new(TestSetup::new(5, consensus_fast_commit_sync));
        let (mut commit_solidifier, selective_dag_state) = setup
            .create_selective_commit_solidifier(
                vec![1, 2, 3, 4, 5],
                vec![(1, 0), (3, 0)], // Exclude first transactions from rounds 1 and 3
            );

        let subdag1 = SubDagBuilder::new(setup.clone(), 1)
            .leader(1, 0)
            .with_blocks(vec![BlockSpec::all_from_round(0)])
            .with_committed_refs(vec![])
            .build();

        let subdag2 = SubDagBuilder::new(setup.clone(), 2)
            .leader(2, 0)
            .with_blocks(vec![BlockSpec::skip_first_from_round(1)])
            .with_committed_refs(vec![])
            .build();

        let subdag3_committed_ref = if consensus_fast_commit_sync {
            GenericTransactionRef::TransactionRef(
                setup.dag_builder.block_headers(1..=1)[0].transaction_ref(),
            )
        } else {
            GenericTransactionRef::from(setup.dag_builder.block_headers(1..=1)[0].reference())
        };

        let subdag3 = SubDagBuilder::new(setup.clone(), 3)
            .leader(4, 0)
            .with_blocks(vec![
                BlockSpec::skip_first_from_round(2),
                BlockSpec::specific_from_round(3, vec![0]),
            ])
            .with_committed_refs(vec![subdag3_committed_ref])
            .build();

        let subdag5_committed_ref = if consensus_fast_commit_sync {
            GenericTransactionRef::TransactionRef(
                setup.dag_builder.block_headers(3..=3)[0].transaction_ref(),
            )
        } else {
            GenericTransactionRef::from(setup.dag_builder.block_headers(3..=3)[0].reference())
        };

        let subdag5 = SubDagBuilder::new(setup.clone(), 5) // Gap: missing index 4
            .leader(5, 0)
            .with_blocks(vec![BlockSpec::skip_first_from_round(4)])
            .with_committed_refs(vec![subdag5_committed_ref])
            .build();

        // Initial commit - should commit first two, buffer the rest
        let (committed, missing) =
            commit_solidifier.try_get_solid_sub_dags(&[subdag1, subdag2, subdag3, subdag5]);
        assert_eq!(committed.len(), 2);
        assert_eq!(missing.len(), 2);
        assert_eq!(commit_solidifier.pending_subdags.len(), 2);
        assert_eq!(commit_solidifier.last_solid_committed_index, 2);

        // Add missing transaction for subdag3
        setup.add_missing_transactions(&selective_dag_state, &[(1, 0)]);
        let (committed, _missing) = commit_solidifier.try_get_solid_sub_dags(&[]);
        assert_eq!(committed.len(), 1); // subdag3 commits
        assert_eq!(commit_solidifier.last_solid_committed_index, 3);

        // Add missing transaction for subdag5
        setup.add_missing_transactions(&selective_dag_state, &[(3, 0)]);
        let (committed, _missing) = commit_solidifier.try_get_solid_sub_dags(&[]);
        assert!(committed.is_empty()); // subdag5 can't commit due to a gap (missing index 4)
        assert_eq!(commit_solidifier.pending_subdags.len(), 1); // subdag5 still pending
        assert_eq!(commit_solidifier.last_solid_committed_index, 3); // Unchanged
    }

    #[rstest]
    #[tokio::test]
    async fn test_set_last_committed_index(
        #[values(false, true)] consensus_fast_commit_sync: bool,
    ) {
        let setup = Arc::new(TestSetup::new(3, consensus_fast_commit_sync));
        let mut commit_solidifier = CommitSolidifier::new(setup.dag_state.clone());

        // Initially should be 0
        assert_eq!(commit_solidifier.last_solid_committed_index, 0);

        // Set to a new value
        commit_solidifier.set_last_solid_committed_index(5);
        assert_eq!(commit_solidifier.last_solid_committed_index, 5);

        // Can set to a lower value
        commit_solidifier.set_last_solid_committed_index(3);
        assert_eq!(commit_solidifier.last_solid_committed_index, 3);

        // Can set to 0
        commit_solidifier.set_last_solid_committed_index(0);
        assert_eq!(commit_solidifier.last_solid_committed_index, 0);
    }

    #[rstest]
    #[tokio::test]
    async fn test_get_missing_transaction_data(
        #[values(false, true)] consensus_fast_commit_sync: bool,
    ) {
        let setup = Arc::new(TestSetup::new(4, consensus_fast_commit_sync));
        let (mut commit_solidifier, selective_dag_state) = setup
            .create_selective_commit_solidifier(
                vec![1, 2, 3, 4],
                vec![(1, 0), (2, 1)], /* Exclude transactions from round 1 block 0 and round 2
                                       * block 1 */
            );

        // Create subdags that reference the missing transactions
        let subdag1_committed_ref = if consensus_fast_commit_sync {
            GenericTransactionRef::TransactionRef(
                setup.dag_builder.block_headers(1..=1)[0].transaction_ref(),
            )
        } else {
            GenericTransactionRef::from(setup.dag_builder.block_headers(1..=1)[0].reference())
        };

        let subdag1 = SubDagBuilder::new(setup.clone(), 1)
            .leader(3, 0)
            .with_blocks(vec![BlockSpec::all_from_round(1)])
            .with_committed_refs(vec![subdag1_committed_ref])
            .build();

        let subdag2_committed_ref = if consensus_fast_commit_sync {
            GenericTransactionRef::TransactionRef(
                setup.dag_builder.block_headers(2..=2)[1].transaction_ref(),
            )
        } else {
            GenericTransactionRef::from(setup.dag_builder.block_headers(2..=2)[1].reference())
        };

        let subdag2 = SubDagBuilder::new(setup.clone(), 2)
            .leader(4, 0)
            .with_blocks(vec![BlockSpec::all_from_round(2)])
            .with_committed_refs(vec![subdag2_committed_ref])
            .build();

        // Add subdags to commit_solidifier
        commit_solidifier.try_get_solid_sub_dags(&[subdag1, subdag2]);

        // Get missing transactions
        let missing = commit_solidifier.get_missing_transaction_data();
        assert_eq!(missing.len(), 2);
        assert!(missing.contains(&subdag1_committed_ref));
        assert!(missing.contains(&subdag2_committed_ref));

        // Add one missing transaction
        setup.add_missing_transactions(&selective_dag_state, &[(1, 0)]);

        // Check missing transactions again
        let missing = commit_solidifier.get_missing_transaction_data();
        assert_eq!(missing.len(), 1);
        assert!(missing.contains(&subdag2_committed_ref));

        // Add the remaining missing transaction
        setup.add_missing_transactions(&selective_dag_state, &[(2, 1)]);

        // Should now have no missing transactions
        let missing = commit_solidifier.get_missing_transaction_data();
        assert!(missing.is_empty());
    }
}
