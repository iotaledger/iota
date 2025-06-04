// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use parking_lot::RwLock;

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
        // TODO: last_committed_index should be initialized from the latest committed
        // subdag, not always zero.
        let last_committed_index = 0;
        Self {
            dag_state,
            pending_subdags: BTreeMap::new(),
            last_committed_index,
        }
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
    /// - `Vec<BlockRef>`: References to missing blocks with missing
    ///   transactions preventing further commits.
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
                    for block_ref in missing_refs {
                        if !missing.insert(block_ref) {
                            panic!("Duplicate missing blockref detected: {:?}", block_ref);
                        }
                    }
                    first_uncommitted_index = Some(next_index);
                    break; // Can't commit further until this one is ready
                }
            }
        }
        // Update last_committed_index
        self.last_committed_index = last_committed;

        // Collect all missing blockrefs from all remaining pending subdags, skipping
        // the first uncommitted (already processed)
        for (idx, subdag) in self.pending_subdags.iter() {
            if Some(*idx) == first_uncommitted_index {
                continue;
            }
            // Query dag_state directly for missing blocks
            let dag_state = self.dag_state.read();
            // TODO: this needs to check for missing transactions, not block headers
            let exists =
                dag_state.contains_block_headers(subdag.committed_transaction_refs.clone());
            for (i, exists) in exists.iter().enumerate() {
                if !exists {
                    let block_ref = subdag.committed_transaction_refs[i];
                    if !missing.insert(block_ref) {
                        panic!("Duplicate missing blockref detected: {:?}", block_ref);
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
        // TODO: this needs to check for missing transactions, not block headers
        let exists = dag_state.contains_block_headers(subdag.committed_transaction_refs.clone());
        let mut missing = Vec::new();
        for (i, exists) in exists.iter().enumerate() {
            if !exists {
                missing.push(subdag.committed_transaction_refs[i]);
            }
        }
        if missing.is_empty() {
            // All blocks exist, so we can create a CommittedSubDag
            // Fetch the VerifiedTransactions for each committed_transaction_ref
            // For now, stub out transaction existence and fetching
            let transactions = subdag
                .committed_transaction_refs
                .iter()
                .map(|_block_ref| {
                    // TODO: Replace with actual transaction fetching from dag_state
                    // For now, just panic or return a stub
                    // panic!("Transaction fetching not implemented");
                    crate::block_header::VerifiedTransactions::new(
                        vec![],
                        *_block_ref,
                        crate::block_header::TransactionsCommitment::default(),
                        bytes::Bytes::new(),
                    )
                })
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
        block_header::{BlockRef, VerifiedBlockHeader},
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

    fn setup_manager_and_dag_with_builder(
        num_rounds: u32,
    ) -> (DataManager, Arc<RwLock<DagState>>, DagBuilder) {
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
    fn test_happy_path_commit_with_dag_builder() {
        let (mut manager, _dag_state, dag_builder) = setup_manager_and_dag_with_builder(2);
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
    #[ignore = "This test is ignored until transaction storage is implemented in DAG state"]
    fn test_missing_blocks_with_dag_builder() {
        let (mut manager, dag_state, mut dag_builder) = setup_manager_and_dag_with_builder(2);
        let block0s = dag_builder.block_headers(0..=0);
        let block2s = dag_builder.block_headers(2..=2);
        let leader = block2s[0].reference();
        // Remove one block from dag_state to simulate missing
        // dag_state
        //     .write()
        //     .recent_blocks
        //     .remove(&block0s[0].reference());
        let committed_refs = block0s.iter().map(|b| b.reference()).collect::<Vec<_>>();
        let mut all_blocks = block2s.clone();
        all_blocks.extend(block0s.clone());
        let subdag = make_pending_subdag(1, leader, all_blocks, committed_refs);
        let (committed, missing) = manager.try_commit(&[subdag]);
        assert!(committed.is_empty());
        assert_eq!(missing, vec![block0s[0].reference()]);
        assert!(manager.pending_subdags.contains_key(&1));
        assert_eq!(manager.last_committed_index, 0);
    }

    #[test]
    #[ignore = "This test is ignored until transaction storage is implemented in DAG state"]
    fn test_commit_after_missing_blocks_arrive_with_dag_builder() {
        let (mut manager, dag_state, mut dag_builder) = setup_manager_and_dag_with_builder(2);
        let block0s = dag_builder.block_headers(0..=0);
        let block2s = dag_builder.block_headers(2..=2);
        let leader = block2s[0].reference();

        let committed_refs = block0s.iter().map(|b| b.reference()).collect::<Vec<_>>();
        let mut all_blocks = block2s.clone();
        all_blocks.extend(block0s.clone());
        let subdag = make_pending_subdag(1, leader, all_blocks, committed_refs);
        let (committed, missing) = manager.try_commit(&[subdag.clone()]);
        assert!(committed.is_empty());
        assert_eq!(missing, vec![block0s[0].reference()]);
        // Add the missing block back
        dag_state.write().accept_block_header(block0s[0].clone());
        let (committed, missing) = manager.try_commit(&[]);
        assert_eq!(committed.len(), 1);
        assert!(missing.is_empty());
        assert!(manager.pending_subdags.is_empty());
        assert_eq!(manager.last_committed_index, 1);
    }

    #[test]
    fn test_multiple_subdags_in_order_with_dag_builder() {
        let (mut manager, _dag_state, dag_builder) = setup_manager_and_dag_with_builder(3);
        let block0s = dag_builder.block_headers(0..=0);
        let block1s = dag_builder.block_headers(1..=1);
        let block2s = dag_builder.block_headers(2..=2);
        let block3s = dag_builder.block_headers(3..=3);
        // subdag1: leader in round 2, committed_refs from round 0
        let subdag1 = make_pending_subdag(
            1,
            block2s[0].reference(),
            {
                // committing all blocks from round 1 and 2
                let mut v = block1s.clone();
                v.extend(block0s.clone());
                // and the leader block from round 2
                v.push(block2s[0].clone());
                v
            },
            block0s.iter().map(|b| b.reference()).collect(),
        );
        // subdag2: leader in round 3, committed_refs from round 1
        let subdag2 = make_pending_subdag(
            2,
            block3s[0].reference(),
            {
                // committing all blocks from round 2 and the leader block from round 3
                let mut v = block2s[1..].to_vec().clone();
                v.push(block3s[0].clone());
                v
            },
            block1s.iter().map(|b| b.reference()).collect(),
        );
        let (committed, missing) = manager.try_commit(&[subdag1, subdag2]);
        assert_eq!(committed.len(), 2);
        assert!(missing.is_empty());
        assert!(manager.pending_subdags.is_empty());
        assert_eq!(manager.last_committed_index, 2);
    }

    #[test]
    fn test_out_of_order_subdags_with_dag_builder() {
        let (mut manager, _dag_state, dag_builder) = setup_manager_and_dag_with_builder(2);
        let block0s = dag_builder.block_headers(0..=0);
        let block1s = dag_builder.block_headers(1..=1);
        let block2s = dag_builder.block_headers(2..=2);
        // subdag2: leader in round 2, committed_refs from round 0
        let subdag2 = make_pending_subdag(
            2,
            block2s[0].reference(),
            {
                let mut v = block2s.clone();
                v.extend(block0s.clone());
                v
            },
            block0s.iter().map(|b| b.reference()).collect(),
        );
        // subdag1: leader in round 1, committed_refs from round 0
        let subdag1 = make_pending_subdag(
            1,
            block1s[0].reference(),
            {
                let mut v = block1s.clone();
                v.extend(block0s.clone());
                v
            },
            block0s.iter().map(|b| b.reference()).collect(),
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
        let (mut manager, _dag_state, _dag_builder) = setup_manager_and_dag_with_builder(2);
        let (committed, missing) = manager.try_commit(&[]);
        assert!(committed.is_empty());
        assert!(missing.is_empty());
        assert!(manager.pending_subdags.is_empty());
        assert_eq!(manager.last_committed_index, 0);
    }

    #[test]
    fn test_duplicate_subdag_commit() {
        let (mut manager, _dag_state, dag_builder) = setup_manager_and_dag_with_builder(2); // Adjusted to 3 rounds
        let block0s = dag_builder.block_headers(0..=0);
        let block1s = dag_builder.block_headers(1..=1);
        let block2s = dag_builder.block_headers(2..=2);

        let subdag1 = make_pending_subdag(
            1,
            block2s[0].reference(),
            {
                let mut v = block1s.clone();
                v.extend(block0s.clone());
                v.push(block2s[0].clone());
                v
            },
            block0s.iter().map(|b| b.reference()).collect(),
        );

        let (committed, missing) = manager.try_commit(&[subdag1.clone(), subdag1.clone()]);
        assert_eq!(committed.len(), 1);
        assert!(missing.is_empty());
        assert!(manager.pending_subdags.is_empty());
        assert_eq!(manager.last_committed_index, 1);
    }

    #[test]
    fn test_out_of_order_commit_calls() {
        let (mut manager, _dag_state, dag_builder) = setup_manager_and_dag_with_builder(3); // Adjusted to 3 rounds
        let block0s = dag_builder.block_headers(0..=0);
        let block1s = dag_builder.block_headers(1..=1);
        let block2s = dag_builder.block_headers(2..=2);
        let block3s = dag_builder.block_headers(3..=3);

        let subdag2 = make_pending_subdag(
            2,
            block3s[0].reference(),
            {
                let mut v = block2s[1..].to_vec().clone();
                v.push(block3s[0].clone());
                v
            },
            block1s.iter().map(|b| b.reference()).collect(),
        );

        let subdag1 = make_pending_subdag(
            1,
            block2s[0].reference(),
            {
                let mut v = block1s.clone();
                v.extend(block0s.clone());
                v.push(block2s[0].clone());
                v
            },
            block0s.iter().map(|b| b.reference()).collect(),
        );

        let (committed, missing) = manager.try_commit(&[subdag2.clone()]);
        assert!(committed.is_empty());
        assert!(missing.is_empty());
        assert!(manager.pending_subdags.contains_key(&2));
        assert_eq!(manager.last_committed_index, 0);

        let (committed, missing) = manager.try_commit(&[subdag1.clone()]);
        assert_eq!(committed.len(), 2);
        assert!(missing.is_empty());
        assert!(manager.pending_subdags.is_empty());
        assert_eq!(manager.last_committed_index, 2);
    }

    #[test]
    #[ignore = "This test is ignored until transaction storage is implemented in DAG state"]
    fn test_all_missing_refs_are_collected() {
        let (mut manager, dag_state, dag_builder) = setup_manager_and_dag_with_builder(4);
        let block0s = dag_builder.block_headers(0..=0);
        let block1s = dag_builder.block_headers(1..=1);
        let block2s = dag_builder.block_headers(2..=2);
        let block3s = dag_builder.block_headers(3..=3);
        let block4s = dag_builder.block_headers(4..=4);

        // Remove some blocks to simulate missing ones
        // let mut state = dag_state.write();
        // state.remove_block(&block0s[0].reference());
        // state.remove_block(&block1s[0].reference());
        // state.remove_block(&block2s[0].reference());
        // drop(state);

        // Create subdags that will be missing different blocks
        let subdag1 = make_pending_subdag(
            1,
            block2s[0].reference(),
            block1s.clone(),
            vec![block0s[0].reference()], // Missing block from round 0
        );

        let subdag2 = make_pending_subdag(
            2,
            block3s[0].reference(),
            block2s.clone(),
            vec![block1s[0].reference()], // Missing block from round 1
        );

        let subdag3 = make_pending_subdag(
            3,
            block4s[0].reference(),
            block3s.clone(),
            vec![block2s[0].reference()], // Missing block from round 2
        );

        // First attempt with subdag3 - highest index
        let (committed, missing) = manager.try_commit(&[subdag3.clone()]);
        assert!(committed.is_empty());
        assert_eq!(missing.len(), 1);
        assert!(missing.contains(&block2s[0].reference()));
        assert_eq!(manager.pending_subdags.len(), 1);

        // Add subdag2
        let (committed, missing) = manager.try_commit(&[subdag2.clone()]);
        assert!(committed.is_empty());
        assert_eq!(missing.len(), 2);
        assert!(missing.contains(&block1s[0].reference()));
        assert!(missing.contains(&block2s[0].reference()));
        assert_eq!(manager.pending_subdags.len(), 2);

        // Add subdag1 - now all missing refs should be collected
        let (committed, missing) = manager.try_commit(&[subdag1.clone()]);
        assert!(committed.is_empty());
        assert_eq!(missing.len(), 3);
        assert!(missing.contains(&block0s[0].reference()));
        assert!(missing.contains(&block1s[0].reference()));
        assert!(missing.contains(&block2s[0].reference()));
        assert_eq!(manager.pending_subdags.len(), 3);

        // Add all missing blocks back
        let mut state = dag_state.write();
        state.accept_block_header(block0s[0].clone());
        state.accept_block_header(block1s[0].clone());
        state.accept_block_header(block2s[0].clone());
        drop(state);

        // Second attempt: all blocks should be committed in order
        let (committed, missing) = manager.try_commit(&[]);
        assert_eq!(committed.len(), 3);
        assert!(missing.is_empty());
        assert!(manager.pending_subdags.is_empty());
        assert_eq!(manager.last_committed_index, 3);
    }

    #[test]
    #[should_panic(expected = "Duplicate missing blockref detected")]
    #[ignore = "This test is ignored until transaction storage is implemented in DAG state"]
    fn test_duplicate_missing_refs_panic() {
        let (mut manager, dag_state, dag_builder) = setup_manager_and_dag_with_builder(3);
        let block0s = dag_builder.block_headers(0..=0);
        let block1s = dag_builder.block_headers(1..=1);
        let block2s = dag_builder.block_headers(2..=2);
        let block3s = dag_builder.block_headers(3..=3);

        // Remove a block that will be referenced by multiple subdags
        // let mut state = dag_state.write();
        // state.remove_block(&block1s[0].reference());
        // drop(state);

        // Create two subdags that reference the same missing block
        let subdag1 = make_pending_subdag(
            1,
            block2s[0].reference(),
            block1s.clone(),
            vec![block0s[0].reference()], // Both subdags reference the same missing block
        );

        let subdag2 = make_pending_subdag(
            2,
            block3s[0].reference(),
            block2s.clone(),
            vec![block0s[0].reference(), block1s[0].reference()], // This should cause a panic
        );

        // This should panic due to duplicate missing block ref
        manager.try_commit(&[subdag1, subdag2]);
    }

    // TODO: Add tests for multiple subdags with the same leader block but different
    // committed_refs  to ensure proper validation of transaction uniqueness
    // across subdags.

    #[test]
    #[ignore = "This test is ignored until transaction storage is implemented in DAG state"]
    fn test_gaps_in_subdags_sequence() {
        let (mut manager, dag_state, dag_builder) = setup_manager_and_dag_with_builder(4);
        let block0s = dag_builder.block_headers(0..=0);
        let block1s = dag_builder.block_headers(1..=1);
        let block2s = dag_builder.block_headers(2..=2);
        let block3s = dag_builder.block_headers(3..=3);
        let block4s = dag_builder.block_headers(4..=4);

        // Remove some blocks to simulate missing ones
        // let mut state = dag_state.write();
        // state.remove_block(&block0s[0].reference());
        // state.remove_block(&block2s[0].reference());
        // drop(state);

        // Create subdags with indices [1, 2, 4], skipping 3
        let subdag1 = make_pending_subdag(
            1,
            block2s[0].reference(),
            block1s.clone(),
            vec![block0s[0].reference()], // Missing block from round 0
        );

        let subdag2 = make_pending_subdag(
            2,
            block3s[0].reference(),
            block2s.clone(),
            vec![block1s[0].reference()], // This block exists
        );

        let subdag4 = make_pending_subdag(
            4, // Note the gap: index 3 is missing
            block4s[0].reference(),
            block3s.clone(),
            vec![block2s[0].reference()], // Missing block from round 2
        );

        // First commit attempt - should only store subdags in buffer since blocks are
        // missing
        let (committed, missing) =
            manager.try_commit(&[subdag1.clone(), subdag2.clone(), subdag4.clone()]);
        assert!(committed.is_empty());
        assert_eq!(missing.len(), 2);
        assert!(missing.contains(&block1s[0].reference()));
        assert_eq!(manager.pending_subdags.len(), 3);
        assert_eq!(manager.last_committed_index, 0);

        // Add missing block from subdag1 and try again - should commit subdags 1 and 2
        dag_state.write().accept_block_header(block0s[0].clone());
        let (committed, missing) = manager.try_commit(&[]);
        assert_eq!(committed.len(), 2); // Should commit subdags 1 and 2
        assert_eq!(missing.len(), 1); // Missing transaction from subdag4
        assert!(missing.contains(&block3s[0].reference()));
        assert_eq!(manager.pending_subdags.len(), 1); // subdag4 should still be pending
        assert_eq!(manager.last_committed_index, 2); // Should stop at 2 due to missing subdag3

        // Try to commit again - should not commit subdag4 due to missing with
        // commit_ref.index=3
        let (committed, missing) = manager.try_commit(&[]);
        assert!(committed.is_empty()); // Nothing should commit
        assert!(missing.is_empty()); // No missing blocks, but still can't commit due to gap
        assert_eq!(manager.pending_subdags.len(), 1); // subdag4 should still be pending
        assert_eq!(manager.last_committed_index, 2); // Should remain at 2

        // subdag4 should remain pending indefinitely until subdag with
        // commit_ref.index=3 arrives
    }
}
