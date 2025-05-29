// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use parking_lot::RwLock;

use crate::{
    BlockRef, CommittedSubDag, commit::PendingSubDag, context::Context, dag_state::DagState,
};

// TODO: write docstring for DataManager
pub(crate) struct DataManager {
    // context: Arc<Context>,
    // dag_state: Arc<RwLock<DagState>>,
}

impl DataManager {
    pub(crate) fn new(_context: Arc<Context>, _dag_state: Arc<RwLock<DagState>>) -> Self {
        Self {
            // context, dag_state
        }
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
