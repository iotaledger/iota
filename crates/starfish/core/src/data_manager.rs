// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use parking_lot::RwLock;

use crate::{
    BlockRef, CommittedSubDag, commit::PendingSubDag, context::Context, dag_state::DagState,
};

/// Block manager suspends incoming blocks until they are connected to the
/// existing graph, returning newly connected blocks.
/// TODO: As it is possible to have Byzantine validators who produce Blocks
/// without valid causal history we need to make sure that BlockManager takes
/// care of that and avoid OOM (Out Of Memory) situations.
pub(crate) struct DataManager {
    context: Arc<Context>,
    dag_state: Arc<RwLock<DagState>>,
}

impl DataManager {
    pub(crate) fn new(context: Arc<Context>, dag_state: Arc<RwLock<DagState>>) -> Self {
        Self { context, dag_state }
    }

    /// Commit the sub-dag to the global state
    pub(crate) fn try_commit(
        &self,
        _p0: &[PendingSubDag],
    ) -> (Vec<CommittedSubDag>, Vec<BlockRef>) {
        todo!()
    }

    pub(crate) fn try_commit_one(&self, _p0: &PendingSubDag) -> Option<CommittedSubDag> {
        todo!()
    }
}
