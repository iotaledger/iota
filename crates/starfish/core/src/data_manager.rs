// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use parking_lot::RwLock;
use tracing::trace;

use crate::{BlockRef, CommittedSubDag, commit::PendingSubDag, dag_state::DagState};

/// The `DataManager` is responsible for managing and handling
/// the commit process for newly committed sub-dags. It ensures that sub-dags
/// are committed after transactions included in the commit are available and
/// that sub-dags are committed in order. The `DataManager` also tracks the
/// highest committed index and maintains a buffer for pending sub-dags for
/// which either the transactions are not yet available or the previous sub-dags
/// are missing transactions and have not been output yet.
///
/// # Fields
/// - `dag_state`: Shared state of the DAG.
/// - `pending_subdags`: Buffer for sub-dags waiting to be committed.
/// - `last_committed_index`: Tracks the highest committed sub-dag index.
///
/// # Usage
/// The `DataManager` is used to process newly committed sub-dags by retrieving
/// information about potentially missing blocks.
pub(crate) struct DataManager {
    dag_state: Arc<RwLock<DagState>>,
    // Buffer for pending subdags, keyed by commit_ref.index for order
    pending_subdags: BTreeMap<u32, PendingSubDag>,
    // The highest committed commit_ref.index
    last_committed_index: u32,
}

impl DataManager {
    /// Creates a new instance of `DataManager`.
    ///
    /// # Arguments
    /// - `dag_state`: Shared state of the DAG.
    ///
    /// # Returns
    /// A new `DataManager` instance.
    pub(crate) fn new(dag_state: Arc<RwLock<DagState>>) -> Self {
        // last_committed_index is set non-trivially during recovery process before the
        // first usage of try_commit method.
        let last_committed_index = 0;
        Self {
            dag_state,
            pending_subdags: BTreeMap::new(),
            last_committed_index,
        }
    }

    pub(crate) fn set_last_committed_index(&mut self, index: u32) {
        self.last_committed_index = index;
    }

    /// Gets all missing transactions from pending subdags.
    ///
    /// # Returns
    /// A `BTreeSet` of `BlockRef`s for which transactions are missing.
    pub(crate) fn get_missing_transaction_data(&self) -> BTreeSet<BlockRef> {
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
    pub(crate) fn try_commit(
        &mut self,
        subdags: &[PendingSubDag],
    ) -> (Vec<CommittedSubDag>, Vec<BlockRef>) {
        // Add new subdags to the buffer
        for subdag in subdags {
            self.pending_subdags
                .entry(subdag.commit_ref.index)
                .or_insert_with(|| subdag.clone());
        }
        let mut committed = Vec::new();
        let mut last_committed = self.last_committed_index;
        let mut missing = BTreeSet::new();
        let mut first_uncommitted_index: Option<u32> = None;
        // Try to commit in order
        loop {
            let next_index = last_committed + 1;
            // If the next expected subdag is not in the buffer, we cannot commit anything
            // further
            let Some(subdag) = self.pending_subdags.get(&next_index) else {
                break;
            };
            match self.try_commit_one_internal(subdag) {
                Ok(committed_subdag) => {
                    committed.push(committed_subdag);
                    self.pending_subdags.remove(&next_index);
                    last_committed = next_index;
                }
                Err(missing_refs) => {
                    // If we have missing refs, we cannot commit this subdag
                    trace!(
                        "Cannot create CommittedSubDag at index {}. Missing refs: {:?}",
                        next_index, missing_refs
                    );

                    first_uncommitted_index = Some(next_index);
                    break; // Can't commit further until this one is ready
                }
            }
        }

        // Update dag state with the round of the leader in the last committed subdag
        // This will allow to evict transactions from the DAG state
        if !committed.is_empty() {
            self.dag_state
                .write()
                .update_last_available_commit_leader_round(
                    committed
                        .last()
                        .expect("We should expect at least one committed subdag")
                        .leader_round(),
                );
        }

        // Update last_committed_index
        self.last_committed_index = last_committed;

        // Only check for missing refs in the newly passed subdags that weren't
        // processed yet
        for subdag in subdags {
            if subdag.commit_ref.index > self.last_committed_index {
                // Query dag_state directly for missing transactions
                let dag_state = self.dag_state.read();
                let exists =
                    dag_state.contains_transactions(subdag.committed_transaction_refs.clone());
                for (i, exists) in exists.iter().enumerate() {
                    if !exists {
                        let block_ref = subdag.committed_transaction_refs[i];
                        if !missing.insert(block_ref) {
                            // Transactions should only be committed by a single subdag, so
                            // duplicates should never happen.
                            panic!("Duplicate missing blockref detected: {:?}", block_ref);
                        }
                    }
                }
            }
        }

        (committed, missing.into_iter().collect())
    }

    /// Internal method to retrieve all committed transactions and checks if all
    /// previous commits have been committed.
    ///
    /// # Arguments
    /// - `subdag`: A reference to the `PendingSubDag` to be committed.
    ///
    /// # Returns
    /// - `Ok(CommittedSubDag)`: If all required blocks exist.
    /// - `Err(Vec<BlockRef>)`: If some blocks are missing.
    fn try_commit_one_internal(
        &self,
        subdag: &PendingSubDag,
    ) -> Result<CommittedSubDag, Vec<BlockRef>> {
        let dag_state = self.dag_state.read();
        // Get transactions and check if any are missing
        let transaction_results = dag_state.get_transactions(&subdag.committed_transaction_refs);
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
                subdag.base.blocks.clone(),
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

    use super::*;
    use crate::{
        block_header::{BlockRef, VerifiedBlockHeader, genesis_blocks},
        commit::{CommitRef, PendingSubDag},
        context::Context,
        dag_state::DagState,
        test_dag_builder::DagBuilder,
    };

    fn make_pending_subdag(
        index: u32,
        leader: BlockRef,
        blocks: Vec<VerifiedBlockHeader>,
        committed_refs: Vec<BlockRef>,
    ) -> PendingSubDag {
        PendingSubDag::new(
            leader,
            blocks,
            committed_refs,
            123456,
            CommitRef {
                index,
                digest: crate::commit::CommitDigest::MIN,
            },
            vec![],
        )
    }

    fn setup_manager_and_dag(num_rounds: u32) -> (DataManager, Arc<RwLock<DagState>>, DagBuilder) {
        let context = Arc::new(Context::new_for_test(2).0);
        let dag_state = Arc::new(RwLock::new(DagState::new(
            context.clone(),
            Arc::new(crate::storage::mem_store::MemStore::new()),
        )));
        let mut dag_builder = DagBuilder::new(context.clone());
        dag_builder
            .layers(1..=num_rounds)
            .build()
            .persist_layers(dag_state.clone());
        let manager = DataManager::new(dag_state.clone());
        (manager, dag_state, dag_builder)
    }

    /// Tests the happy path where a single sub-dag is successfully committed.
    #[test]
    fn test_happy_path_commit() {
        let (mut manager, _dag_state, dag_builder) = setup_manager_and_dag(2);
        // Use blocks from round 0 and 2
        let block0s = dag_builder.block_headers(0..=0);
        let block2s = dag_builder.block_headers(2..=2);
        let leader = block2s[0].reference();
        // committed_refs from round 0 (R-2)
        let committed_refs = block0s.iter().map(|b| b.reference()).collect::<Vec<_>>();
        let mut all_blocks = block2s.clone();
        all_blocks.extend(block0s.clone());
        let subdag = make_pending_subdag(1, leader, all_blocks, committed_refs);
        let (committed, missing) = manager.try_commit(&[subdag]);
        assert_eq!(committed.len(), 1);
        assert!(missing.is_empty());
        assert_eq!(manager.last_committed_index, 1);
        assert!(manager.pending_subdags.is_empty());
    }

    #[test]
    fn test_missing_blocks() {
        // Create a shared context for the test
        let context = Arc::new(Context::new_for_test(2).0);

        // Create a DAG state with the context
        let original_dag_state = Arc::new(RwLock::new(DagState::new(
            context.clone(),
            Arc::new(crate::storage::mem_store::MemStore::new()),
        )));

        // Create a DAG builder with the same context
        let mut dag_builder = DagBuilder::new(context.clone());

        // Build the DAG with 2 rounds and persist it to the original DAG state
        dag_builder
            .layers(1..=3)
            .build()
            .persist_layers(original_dag_state.clone());

        // Create genesis blocks

        // Skip adding genesis blocks directly to the DAG state
        // Genesis blocks should be handled separately if needed

        // Get blocks from each round
        let block1s = dag_builder.block_headers(1..=1);
        let block2s = dag_builder.block_headers(2..=2);
        let block3s = dag_builder.block_headers(3..=3);

        // Ensure we have blocks in each round
        assert!(
            !block1s.is_empty(),
            "Expected at least one block in round 1"
        );
        assert!(
            !block2s.is_empty(),
            "Expected at least one block in round 2"
        );
        assert!(
            !block3s.is_empty(),
            "Expected at least one block in round 0"
        );
        // Create a new empty DAG state with the same context
        let selective_dag_state = Arc::new(RwLock::new(DagState::new(
            context.clone(),
            Arc::new(crate::storage::mem_store::MemStore::new()),
        )));

        // Add only blocks from rounds 1 and 2 (excluding genesis blocks)
        let mut state = selective_dag_state.write();
        state.accept_block_headers(block1s.clone());
        state.accept_block_headers(block2s.clone());
        state.accept_block_headers(block3s.clone());
        drop(state);

        // Create DataManager with the selective DAG
        let mut manager = DataManager::new(selective_dag_state.clone());

        // Create a subdag that references the missing block
        let leader = block3s[0].reference();
        let committed_refs = vec![block1s[0].reference()];
        let mut all_blocks = block3s.clone();
        all_blocks.extend(block1s.clone());
        all_blocks.extend(block2s.clone());
        let subdag = make_pending_subdag(1, leader, all_blocks, committed_refs);

        // Try to commit - should fail due to missing block
        let (committed, missing) = manager.try_commit(&[subdag]);
        assert!(
            committed.is_empty(),
            "Expected no committed subdags, got: {:?}",
            committed
        );
        assert_eq!(
            missing,
            vec![block1s[0].reference()],
            "Expected missing block from round 1"
        );
        assert_eq!(
            manager.pending_subdags.len(),
            1,
            "Expected 1 pending subdag, got: {:?}",
            manager.pending_subdags
        );
        assert_eq!(
            manager.last_committed_index, 0,
            "Expected last committed index to be 0, got: {}",
            manager.last_committed_index
        );
    }

    #[test]
    fn test_commit_after_missing_blocks_arrive() {
        // Create a shared context for the test
        let context = Arc::new(Context::new_for_test(2).0);

        // Create a DAG state with the context
        let original_dag_state = Arc::new(RwLock::new(DagState::new(
            context.clone(),
            Arc::new(crate::storage::mem_store::MemStore::new()),
        )));

        // Create a DAG builder with the same context
        let mut dag_builder = DagBuilder::new(context.clone());

        // Build the DAG with 2 rounds and persist it to the original DAG state
        dag_builder
            .layers(1..=3)
            .build()
            .persist_layers(original_dag_state.clone());

        // Get blocks from each round
        let block0s = genesis_blocks(context.clone());
        let block1s = dag_builder.blocks(1..=1);
        let block2s = dag_builder.blocks(2..=2);
        let block3s = dag_builder.blocks(3..=3);

        // Ensure we have blocks in each round
        assert!(
            !block1s.is_empty(),
            "Expected at least one block in round 1"
        );
        assert!(
            !block2s.is_empty(),
            "Expected at least one block in round 2"
        );
        assert!(
            !block3s.is_empty(),
            "Expected at least one block in round 3"
        );
        // Create a new empty DAG state with the same context
        let selective_dag_state = Arc::new(RwLock::new(DagState::new(
            context.clone(),
            Arc::new(crate::storage::mem_store::MemStore::new()),
        )));

        // Add only blocks from rounds 1, 2 and 3 (excluding genesis blocks)
        let mut state = selective_dag_state.write();
        state.accept_block_headers(
            block1s
                .iter()
                .map(|block| block.verified_block_header.clone())
                .collect(),
        );
        state.accept_block_headers(
            block2s
                .iter()
                .map(|block| block.verified_block_header.clone())
                .collect(),
        );
        state.accept_block_headers(
            block3s
                .iter()
                .map(|block| block.verified_block_header.clone())
                .collect(),
        );
        drop(state);

        // Create DataManager with the selective DAG
        let mut manager = DataManager::new(selective_dag_state.clone());

        // Create a subdag that references the missing block
        let leader = block3s[0].reference();
        let committed_refs = vec![block1s[0].reference()];
        let mut all_blocks = vec![block3s[0].verified_block_header.clone()];
        all_blocks.extend(
            block2s
                .iter()
                .chain(block1s.iter())
                .map(|block| block.verified_block_header.clone())
                .collect::<Vec<_>>(),
        );
        let subdag = make_pending_subdag(1, leader, all_blocks, committed_refs);

        // First attempt should fail due to missing block
        let (committed, missing) = manager.try_commit(&[subdag.clone()]);
        assert!(committed.is_empty());
        assert_eq!(missing, vec![block1s[0].reference()]);

        // Add the missing block to the selective DAG
        selective_dag_state
            .write()
            .add_transactions(block1s[0].verified_transactions.clone());

        // The second attempt should succeed
        let (committed, missing) = manager.try_commit(&[]);
        assert_eq!(committed.len(), 1);
        assert!(missing.is_empty());
        assert!(manager.pending_subdags.is_empty());
        assert_eq!(manager.last_committed_index, 1);
    }

    #[test]
    fn test_multiple_subdags_in_order() {
        let (mut manager, _dag_state, dag_builder) = setup_manager_and_dag(4);
        let block1s = dag_builder.block_headers(1..=1);
        let block2s = dag_builder.block_headers(2..=2);
        let block3s = dag_builder.block_headers(3..=3);
        let block4s = dag_builder.block_headers(3..=3);

        // subdag1: leader in round 3, committed_refs from round 1
        let subdag1 = make_pending_subdag(
            1,
            block3s[0].reference(),
            {
                // committing all blocks from round 1 and 2
                let mut v = block2s.clone();
                v.extend(block1s.clone());
                // and the leader block from round 2
                v.push(block3s[0].clone());
                v
            },
            block1s.iter().map(|b| b.reference()).collect(),
        );
        // subdag2: leader in round 4, committed_refs from round 2
        let subdag2 = make_pending_subdag(
            2,
            block4s[0].reference(),
            {
                // committing all blocks from round 2 and the leader block from round 4
                let mut v = block3s[1..].to_vec().clone();
                v.push(block4s[0].clone());
                v
            },
            block2s.iter().map(|b| b.reference()).collect(),
        );
        let (committed, missing) = manager.try_commit(&[subdag1, subdag2]);
        assert_eq!(
            committed.len(),
            2,
            "Expected 2 subdags to be committed, got: {:?}",
            committed
        );
        assert!(missing.is_empty());
        assert!(manager.pending_subdags.is_empty());
        assert_eq!(manager.last_committed_index, 2);
    }

    #[test]
    fn test_out_of_order_subdags() {
        let (mut manager, _dag_state, dag_builder) = setup_manager_and_dag(3);
        let block1s = dag_builder.block_headers(1..=1);
        let block2s = dag_builder.block_headers(2..=2);
        let block3s = dag_builder.block_headers(3..=3);
        // subdag2: leader in round 3, committed_refs from round 2
        let subdag2 = make_pending_subdag(
            2,
            block3s[0].reference(),
            {
                let mut v = block2s[1..].to_vec().clone();
                v.push(block3s[0].clone());
                v
            },
            block2s.iter().map(|b| b.reference()).collect(),
        );
        // subdag1: leader in round 2, committed_refs from round 1
        let subdag1 = make_pending_subdag(
            1,
            block2s[0].reference(),
            {
                let mut v = block1s.clone();
                v.push(block2s[0].clone());
                v
            },
            block2s.iter().map(|b| b.reference()).collect(),
        );
        let (committed, missing) = manager.try_commit(&[subdag2.clone(), subdag1.clone()]);
        assert_eq!(committed.len(), 2);
        assert!(missing.is_empty());
        assert!(manager.pending_subdags.is_empty());
        assert_eq!(manager.last_committed_index, 2);
        let (committed, missing) = manager.try_commit(&[]);
        assert!(committed.is_empty());
        assert!(missing.is_empty());
        assert!(manager.pending_subdags.is_empty());
        assert_eq!(manager.last_committed_index, 2);
    }

    #[test]
    fn test_empty_subdag_commit() {
        let (mut manager, _dag_state, _dag_builder) = setup_manager_and_dag(2);
        let (committed, missing) = manager.try_commit(&[]);
        assert!(committed.is_empty());
        assert!(missing.is_empty());
        assert!(manager.pending_subdags.is_empty());
        assert_eq!(manager.last_committed_index, 0);
    }

    #[test]
    fn test_duplicate_subdag_commit() {
        let (mut manager, _dag_state, dag_builder) = setup_manager_and_dag(3);
        let block1s = dag_builder.block_headers(1..=1);
        let block2s = dag_builder.block_headers(2..=2);
        let block3s = dag_builder.block_headers(2..=2);

        let subdag1 = make_pending_subdag(
            1,
            block3s[0].reference(),
            {
                let mut v = block1s.clone();
                v.extend(block2s.clone());
                v.push(block3s[0].clone());
                v
            },
            block1s.iter().map(|b| b.reference()).collect(),
        );

        let (committed, missing) = manager.try_commit(&[subdag1.clone(), subdag1.clone()]);
        assert_eq!(committed.len(), 1);
        assert!(missing.is_empty());
        assert!(manager.pending_subdags.is_empty());
        assert_eq!(manager.last_committed_index, 1);
    }

    #[test]
    fn test_out_of_order_commit_calls() {
        let (mut manager, _dag_state, dag_builder) = setup_manager_and_dag(4);
        let block1s = dag_builder.block_headers(1..=1);
        let block2s = dag_builder.block_headers(2..=2);
        let block3s = dag_builder.block_headers(3..=3);
        let block4s = dag_builder.block_headers(4..=4);

        let subdag2 = make_pending_subdag(
            2,
            block4s[0].reference(),
            {
                let mut v = block3s[1..].to_vec().clone();
                v.push(block4s[0].clone());
                v
            },
            block2s.iter().map(|b| b.reference()).collect(),
        );

        let subdag1 = make_pending_subdag(
            1,
            block3s[0].reference(),
            {
                let mut v = block1s.clone();
                v.extend(block2s.clone());
                v.push(block3s[0].clone());
                v
            },
            block1s.iter().map(|b| b.reference()).collect(),
        );

        let (committed, missing) = manager.try_commit(&[subdag2.clone()]);
        assert!(
            committed.is_empty(),
            "Expected no committed subdags, got: {:?}",
            committed
        );
        assert!(
            missing.is_empty(),
            "Expected no missing blocks, got: {:?}",
            missing
        );
        assert!(
            manager.pending_subdags.contains_key(&2),
            "Expected pending subdag for index 2, got: {:?}",
            manager.pending_subdags
        );
        assert_eq!(
            manager.last_committed_index, 0,
            "Expected last committed index to be 0, got: {}",
            manager.last_committed_index
        );

        let (committed, missing) = manager.try_commit(&[subdag1.clone()]);
        assert_eq!(
            committed.len(),
            2,
            "Expected 2 subdags to be committed, got: {:?}",
            committed
        );
        assert!(
            missing.is_empty(),
            "Expected no missing blocks, got: {:?}",
            missing
        );
        assert!(
            manager.pending_subdags.is_empty(),
            "Expected no pending subdags, got: {:?}",
            manager.pending_subdags
        );
        assert_eq!(
            manager.last_committed_index, 2,
            "Expected last committed index to be 2, got: {}",
            manager.last_committed_index
        );
    }

    // Test to ensure that all pending subdags are committed correctly once all
    // missing transactions become available.
    #[test]
    fn test_all_missing_refs_are_collected() {
        telemetry_subscribers::init_for_testing();

        // Create a shared context for the test
        let context = Arc::new(Context::new_for_test(2).0);

        // Create a DAG state with the context
        let original_dag_state = Arc::new(RwLock::new(DagState::new(
            context.clone(),
            Arc::new(crate::storage::mem_store::MemStore::new()),
        )));

        // Create a DAG builder with the same context
        let mut dag_builder = DagBuilder::new(context.clone());

        // Build the DAG with 4 rounds and persist it to the original full DAG state.
        dag_builder
            .layers(1..=4)
            .build()
            .persist_layers(original_dag_state.clone());

        // Get blocks from each round
        let block1s = dag_builder.blocks(1..=1);
        let block2s = dag_builder.blocks(2..=2);
        let block3s = dag_builder.blocks(3..=3);
        let block4s = dag_builder.blocks(4..=4);

        // Ensure we have blocks in each round
        assert!(
            !block1s.is_empty(),
            "Expected at least one block in round 1"
        );
        assert!(
            !block2s.is_empty(),
            "Expected at least one block in round 2"
        );
        assert!(
            !block3s.is_empty(),
            "Expected at least one block in round 3"
        );
        assert!(
            !block4s.is_empty(),
            "Expected at least one block in round 4"
        );

        // Create a new empty DAG state with the same context. This DAG State will be
        // used to selectively add committed transactions to simulate missing
        // transactions.
        let selective_dag_state = Arc::new(RwLock::new(DagState::new(
            context.clone(),
            Arc::new(crate::storage::mem_store::MemStore::new()),
        )));

        let mut state = selective_dag_state.write();
        // Add all blocks except the ones we want to exclude (block1s[0], block2s[0])
        for (i, block) in block1s.iter().enumerate() {
            state.accept_block_header(block.verified_block_header.clone());
            if i != 0 {
                state.add_transactions(block.verified_transactions.clone());
            }
        }
        for (i, block) in block2s.iter().enumerate() {
            state.accept_block_header(block.verified_block_header.clone());
            if i != 0 {
                state.add_transactions(block.verified_transactions.clone());
            }
        }
        block3s.iter().for_each(|block| {
            state.accept_block_header(block.verified_block_header.clone());
            state.add_transactions(block.verified_transactions.clone());
        });
        block4s.iter().for_each(|block| {
            state.accept_block_header(block.verified_block_header.clone());
            state.add_transactions(block.verified_transactions.clone());
        });
        drop(state);

        // Create DataManager with the selective DAG
        let mut manager = DataManager::new(selective_dag_state.clone());

        // Create subdags that will be missing different blocks
        let subdag1 = make_pending_subdag(
            1,
            block2s[0].reference(),
            {
                // Committing all blocks from round 1 and 2
                let mut v = block1s.clone();
                v.extend(genesis_blocks(context.clone()));
                v.push(block2s[0].clone());
                v.iter()
                    .map(|block| block.verified_block_header.clone())
                    .collect()
            },
            vec![], // No committed refs from round 0
        );

        let subdag2 = make_pending_subdag(
            2,
            block3s[0].reference(),
            {
                let mut v = block2s[1..].to_vec().clone();
                v.push(block3s[0].clone());
                v.iter()
                    .map(|block| block.verified_block_header.clone())
                    .collect()
            },
            vec![block1s[0].reference()], // Missing block from round 1
        );

        let subdag3 = make_pending_subdag(
            3,
            block4s[0].reference(),
            {
                let mut v = block3s[1..].to_vec().clone();
                v.push(block4s[0].clone());
                v.iter()
                    .map(|block| block.verified_block_header.clone())
                    .collect()
            },
            vec![block2s[0].reference()], // Missing block from round 2
        );

        // First attempt with subdag3 - highest index
        let (committed, missing) = manager.try_commit(&[subdag3.clone()]);
        assert!(
            committed.is_empty(),
            "Expected no subdags to be committed, got: {:?}",
            committed
        );
        assert_eq!(
            missing.len(),
            1,
            "Expected 1 missing block, got: {:?}",
            missing
        );
        assert!(
            missing.contains(&block2s[0].reference()),
            "Expected missing block from round 2"
        );
        assert_eq!(
            manager.pending_subdags.len(),
            1,
            "Expected 1 pending subdag, got: {:?}",
            manager.pending_subdags
        );

        // Add subdag2
        let (committed, missing) = manager.try_commit(&[subdag2.clone()]);
        assert!(
            committed.is_empty(),
            "Expected no subdags to be committed, got: {:?}",
            committed
        );
        assert_eq!(
            missing.len(),
            1,
            "Expected 1 missing block, got: {:?}",
            missing
        );
        assert!(
            missing.contains(&block1s[0].reference()),
            "Expected missing block from round 1"
        );
        assert_eq!(
            manager.pending_subdags.len(),
            2,
            "Expected 2 pending subdags, got: {:?}",
            manager.pending_subdags
        );

        // Add subdag1 - no missing transacions so it should be committed.
        let (committed, missing) = manager.try_commit(&[subdag1.clone()]);
        assert_eq!(
            committed.len(),
            1,
            "Expected 1 subdag to be committed, got: {:?}",
            committed
        );
        assert_eq!(
            committed[0].commit_ref, subdag1.commit_ref,
            "Expected subdag1 to be committed"
        );
        assert_eq!(
            missing.len(),
            0,
            "Expected no missing blocks, got: {:?}",
            missing
        );
        assert_eq!(
            manager.pending_subdags.len(),
            2,
            "Expected 2 pending subdags, got: {:?}",
            manager.pending_subdags
        );

        // Add missing blocks from Round 1 back to the selective DAG
        let mut state = selective_dag_state.write();
        state.add_transactions(block1s[0].verified_transactions.clone());
        drop(state);

        // Second attempt: subdag2 should be committed now, subdag3 still missing one
        // transaction
        let (committed, missing) = manager.try_commit(&[]);
        assert_eq!(
            committed.len(),
            1,
            "Expected 1 subdag to be committed, got: {:?}",
            committed
        );
        assert_eq!(
            committed[0].commit_ref, subdag2.commit_ref,
            "Expected subdag2 to be committed"
        );
        assert!(
            missing.is_empty(),
            "Expected no missing blocks, got: {:?}",
            missing
        );
        assert_eq!(
            manager.pending_subdags.len(),
            1,
            "Expected 1 pending subdag, got: {:?}",
            manager.pending_subdags
        );
        assert_eq!(
            manager.last_committed_index, 2,
            "Expected last committed index to be 2, got: {}",
            manager.last_committed_index
        );

        let mut state = selective_dag_state.write();
        state.add_transactions(block2s[0].verified_transactions.clone());
        drop(state);

        // Third attempt: subdag3 should be committed now, no more pending subdags
        let (committed, missing) = manager.try_commit(&[]);
        assert_eq!(
            committed.len(),
            1,
            "Expected 1 subdag to be committed, got: {:?}",
            committed
        );
        assert_eq!(
            committed[0].commit_ref, subdag3.commit_ref,
            "Expected subdag3 to be committed"
        );
        assert!(
            missing.is_empty(),
            "Expected no missing blocks, got: {:?}",
            missing
        );
        assert_eq!(
            manager.pending_subdags.len(),
            0,
            "Expected no pending subdags, got: {:?}",
            manager.pending_subdags
        );
        assert_eq!(
            manager.last_committed_index, 3,
            "Expected last committed index to be 3, got: {}",
            manager.last_committed_index
        );
    }

    #[test]
    #[should_panic(expected = "Duplicate missing blockref detected")]
    fn test_duplicate_missing_refs_panic() {
        // Create a shared context for the test
        let context = Arc::new(Context::new_for_test(2).0);

        // Create a DAG state with the context
        let original_dag_state = Arc::new(RwLock::new(DagState::new(
            context.clone(),
            Arc::new(crate::storage::mem_store::MemStore::new()),
        )));

        // Create a DAG builder with the same context
        let mut dag_builder = DagBuilder::new(context.clone());

        // Build the DAG with 3 rounds and persist it to the original DAG state
        dag_builder
            .layers(1..=4)
            .build()
            .persist_layers(original_dag_state.clone());

        // Create genesis blocks
        let block0s = crate::block_header::genesis_block_headers(context.clone());
        let block1s = dag_builder.blocks(1..=1);
        let block2s = dag_builder.blocks(2..=2);
        let block3s = dag_builder.blocks(3..=3);
        let block4s = dag_builder.blocks(4..=4);

        // Ensure we have blocks in each round
        assert!(
            !block0s.is_empty(),
            "Expected at least one block in round 0"
        );
        assert!(
            !block1s.is_empty(),
            "Expected at least one block in round 1"
        );
        assert!(
            !block2s.is_empty(),
            "Expected at least one block in round 2"
        );
        assert!(
            !block3s.is_empty(),
            "Expected at least one block in round 3"
        );
        assert!(
            !block4s.is_empty(),
            "Expected at least one block in round 4"
        );
        // Create a new empty DAG state with the same context
        let selective_dag_state = Arc::new(RwLock::new(DagState::new(
            context.clone(),
            Arc::new(crate::storage::mem_store::MemStore::new()),
        )));

        // Add all blocks except block1s[0]
        let mut state = selective_dag_state.write();
        for (i, block) in block1s
            .iter()
            .chain(block2s.iter())
            .chain(block3s.iter())
            .chain(block4s.iter())
            .enumerate()
        {
            state.accept_block_header(block.verified_block_header.clone());
            if i != 0 {
                state.add_transactions(block.verified_transactions.clone());
            }
        }
        drop(state);
        // Create DataManager with the selective DAG
        let mut manager = DataManager::new(selective_dag_state.clone());

        // Create first subdag that does not commit any transactions
        let subdag1 = make_pending_subdag(
            1,
            block2s[0].reference(),
            {
                let mut v: Vec<_> = block1s
                    .clone()
                    .iter()
                    .map(|block| block.verified_block_header.clone())
                    .collect();
                v.extend(block0s.clone());
                v.push(block2s[0].verified_block_header.clone());
                v
            },
            vec![], // No committed refs from round 0
        );
        // Next, create two subdags that reference the same missing block
        let subdag2 = make_pending_subdag(
            2,
            block3s[0].reference(),
            {
                let mut v = block1s[1..].to_vec().clone(); // Skip the first block in block1s as it was the leader in the previous subdag
                v.push(block3s[0].clone());
                v.iter()
                    .map(|block| block.verified_block_header.clone())
                    .collect()
            },
            vec![block1s[0].reference()], // This should cause a panic
        );
        let subdag3 = make_pending_subdag(
            2,
            block4s[0].reference(),
            {
                let mut v = block3s[1..].to_vec().clone(); // Skip the first block in block1s as it was the leader in the previous subdag
                v.push(block4s[0].clone());
                v.iter()
                    .map(|block| block.verified_block_header.clone())
                    .collect()
            },
            vec![block1s[0].reference(), block2s[0].reference()], // This should cause a panic
        );

        // This should panic due to duplicate missing block ref.
        // This only panics when there are duplicate missing block refs in the passed
        // subdags. If there are missing blocks in the pending subdags, they will not
        // cause a panic.
        manager.try_commit(&[subdag1, subdag2, subdag3]);
    }

    // TODO: Add tests for multiple subdags with the same leader block but different
    // committed_refs  to ensure proper validation of transaction uniqueness
    // across subdags.

    // Test to ensure that gaps in the sequence of subdags are handled correctly.
    // Whenever
    #[test]
    fn test_gaps_in_subdags_sequence() {
        // Create a shared context for the test
        let context = Arc::new(Context::new_for_test(2).0);

        // Create a DAG state with the context
        let original_dag_state = Arc::new(RwLock::new(DagState::new(
            context.clone(),
            Arc::new(crate::storage::mem_store::MemStore::new()),
        )));

        // Create a DAG builder with the same context
        let mut dag_builder = DagBuilder::new(context.clone());

        // Build the DAG with 5 rounds and persist it to the original DAG state
        dag_builder
            .layers(1..=5)
            .build()
            .persist_layers(original_dag_state.clone());

        // Get blocks from each round
        let block0s = genesis_blocks(context.clone());
        let block1s = dag_builder.blocks(1..=1);
        let block2s = dag_builder.blocks(2..=2);
        let block3s = dag_builder.blocks(3..=3);
        let block4s = dag_builder.blocks(4..=4);
        let block5s = dag_builder.blocks(5..=5);

        // Ensure we have blocks in each round
        assert!(
            !block1s.is_empty(),
            "Expected at least one block in round 1"
        );
        assert!(
            !block2s.is_empty(),
            "Expected at least one block in round 2"
        );
        assert!(
            !block3s.is_empty(),
            "Expected at least one block in round 3"
        );
        assert!(
            !block4s.is_empty(),
            "Expected at least one block in round 4"
        );
        assert!(
            !block5s.is_empty(),
            "Expected at least one block in round 5"
        );

        // Create a new empty DAG state with the same context
        let selective_dag_state = Arc::new(RwLock::new(DagState::new(
            context.clone(),
            Arc::new(crate::storage::mem_store::MemStore::new()),
        )));
        let mut state = selective_dag_state.write();

        // Add all blocks except block1s[0] and block3s[0]
        for (i, block) in block1s.iter().enumerate() {
            state.accept_block_header(block.verified_block_header.clone());
            if i != 0 {
                state.add_transactions(block.verified_transactions.clone());
            }
        }
        block2s.iter().for_each(|block| {
            state.accept_block_header(block.verified_block_header.clone());
            state.add_transactions(block.verified_transactions.clone());
        });
        for (i, block) in block3s.iter().enumerate() {
            state.accept_block_header(block.verified_block_header.clone());
            if i != 0 {
                state.add_transactions(block.verified_transactions.clone());
            }
        }
        block4s.iter().for_each(|block| {
            state.accept_block_header(block.verified_block_header.clone());
            state.add_transactions(block.verified_transactions.clone());
        });
        block5s.iter().for_each(|block| {
            state.accept_block_header(block.verified_block_header.clone());
            state.add_transactions(block.verified_transactions.clone());
        });
        drop(state);

        // Create DataManager with the selective DAG
        let mut manager = DataManager::new(selective_dag_state.clone());

        // Create subdags with indices [1, 2, 3, 5], skipping 4
        let subdag1 = make_pending_subdag(
            1,
            block1s[0].reference(),
            {
                let mut v: Vec<_> = block0s
                    .iter()
                    .map(|block| block.verified_block_header.clone())
                    .collect();
                v.push(block1s[0].verified_block_header.clone());
                v
            },
            vec![], // No committed refs from round 0
        );

        let subdag2 = make_pending_subdag(
            2,
            block2s[0].reference(),
            {
                let mut v = block1s[1..].to_vec().clone(); // Skip the first block in block1s as it was the leader in the previous subdag
                v.push(block2s[0].clone());
                v.iter()
                    .map(|block| block.verified_block_header.clone())
                    .collect()
            },
            vec![], // No committed refs from round 1 or 0
        );
        let subdag3 = make_pending_subdag(
            3,
            block4s[0].reference(),
            {
                let mut v = block2s[1..].to_vec().clone(); // Skip the first block in block2s as it was the leader in the previous subdag
                v.push(block3s[0].clone());
                v.iter()
                    .map(|block| block.verified_block_header.clone())
                    .collect()
            },
            vec![block1s[0].reference()], /* This is the missing transactions reference from
                                           * round 1 */
        );

        let subdag5 = make_pending_subdag(
            5, // Note the gap: index 3 is missing
            block5s[0].reference(),
            {
                let mut v = block4s[1..].to_vec().clone(); // Skip the first block in block4s as it would the leader in the previous subdag
                v.push(block5s[0].clone());
                v.iter()
                    .map(|block| block.verified_block_header.clone())
                    .collect()
            },
            vec![block3s[0].reference()], // Missing block from round 3
        );

        // First commit attempt - should only store subdags in buffer since blocks are
        // missing
        let (committed, missing) = manager.try_commit(&[
            subdag1.clone(),
            subdag2.clone(),
            subdag3.clone(),
            subdag5.clone(),
        ]);
        assert_eq!(committed.len(), 2);
        assert_eq!(missing.len(), 2);
        assert!(missing.contains(&block3s[0].reference()));
        assert!(missing.contains(&block1s[0].reference()));
        assert_eq!(manager.pending_subdags.len(), 2);
        assert_eq!(manager.last_committed_index, 2);

        // Add the missing block from subdag3 and try again - should commit subdag 3
        selective_dag_state
            .write()
            .add_transactions(block1s[0].verified_transactions.clone());
        let (committed, missing) = manager.try_commit(&[]);
        assert_eq!(committed.len(), 1); // Committed subdag3 because it now has all
        // transactions and past subdags
        assert!(missing.is_empty()); // No new missing blocks
        assert_eq!(manager.pending_subdags.len(), 1); // subdag5 should still be pending
        assert_eq!(manager.last_committed_index, 3);

        // Add the second missing block
        selective_dag_state
            .write()
            .add_transactions(block3s[0].verified_transactions.clone());

        // Try to commit again - should not commit subdag5 due to missing with
        // commit_ref.index=4
        let (committed, missing) = manager.try_commit(&[]);
        assert!(committed.is_empty()); // Nothing should commit
        assert!(missing.is_empty()); // No missing blocks, but still can't commit due to gap
        assert_eq!(manager.pending_subdags.len(), 1); // subdag5 should still be pending
        assert_eq!(manager.last_committed_index, 3); // Should remain at 3

        // subdag4 should remain pending indefinitely until subdag with
        // commit_ref.index=3 arrives
    }
}
