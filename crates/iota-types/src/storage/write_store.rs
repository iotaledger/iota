// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use super::error::Result;
use crate::{
    committee::Committee,
    messages_checkpoint::{
        CheckpointSequenceNumber, VerifiedCheckpoint, VerifiedCheckpointContents,
    },
    storage::ReadStore,
};

pub trait WriteStore: ReadStore {
    fn try_insert_checkpoint(&self, checkpoint: &VerifiedCheckpoint) -> Result<()>;

    /// Non-fallible version of `try_insert_checkpoint`.
    fn insert_checkpoint(&self, checkpoint: &VerifiedCheckpoint) {
        self.try_insert_checkpoint(checkpoint)
            .expect("storage access failed")
    }

    fn try_update_highest_synced_checkpoint(&self, checkpoint: &VerifiedCheckpoint) -> Result<()>;

    /// Non-fallible version of `try_update_highest_synced_checkpoint`.
    fn update_highest_synced_checkpoint(&self, checkpoint: &VerifiedCheckpoint) {
        self.try_update_highest_synced_checkpoint(checkpoint)
            .expect("storage access failed")
    }

    fn try_update_highest_verified_checkpoint(&self, checkpoint: &VerifiedCheckpoint)
    -> Result<()>;

    /// Non-fallible version of `try_update_highest_verified_checkpoint`.
    fn update_highest_verified_checkpoint(&self, checkpoint: &VerifiedCheckpoint) {
        self.try_update_highest_verified_checkpoint(checkpoint)
            .expect("storage access failed")
    }

    fn try_insert_checkpoint_contents(
        &self,
        checkpoint: &VerifiedCheckpoint,
        contents: VerifiedCheckpointContents,
    ) -> Result<()>;

    /// Non-fallible version of `try_insert_checkpoint_contents`.
    fn insert_checkpoint_contents(
        &self,
        checkpoint: &VerifiedCheckpoint,
        contents: VerifiedCheckpointContents,
    ) {
        self.try_insert_checkpoint_contents(checkpoint, contents)
            .expect("storage access failed")
    }

    fn try_insert_committee(&self, new_committee: Committee) -> Result<()>;

    /// Non-fallible version of `try_insert_committee`.
    fn insert_committee(&self, new_committee: Committee) {
        self.try_insert_committee(new_committee)
            .expect("storage access failed")
    }

    /// The highest checkpoint whose transactions have been executed, when the
    /// store tracks execution progress. `None` when nothing has been executed
    /// yet or the store does not track execution (the default).
    fn try_get_highest_executed_checkpoint_seq_number(
        &self,
    ) -> Result<Option<CheckpointSequenceNumber>> {
        Ok(None)
    }

    /// Non-fallible version of
    /// `try_get_highest_executed_checkpoint_seq_number`.
    fn get_highest_executed_checkpoint_seq_number(&self) -> Option<CheckpointSequenceNumber> {
        self.try_get_highest_executed_checkpoint_seq_number()
            .expect("storage access failed")
    }

    /// Inserts a consecutive run of verified checkpoints with their full
    /// contents and advances the verified and synced watermarks past the
    /// last one.
    ///
    /// The default implementation inserts the checkpoints one at a time;
    /// implementations can override it to batch the writes.
    fn try_insert_synced_checkpoints(
        &self,
        checkpoints: Vec<(VerifiedCheckpoint, VerifiedCheckpointContents)>,
    ) -> Result<()> {
        for (checkpoint, contents) in checkpoints {
            self.try_insert_checkpoint(&checkpoint)?;
            self.try_update_highest_verified_checkpoint(&checkpoint)?;
            self.try_insert_checkpoint_contents(&checkpoint, contents)?;
            self.try_update_highest_synced_checkpoint(&checkpoint)?;
        }
        Ok(())
    }
}

impl<T: WriteStore + ?Sized> WriteStore for &T {
    fn try_insert_checkpoint(&self, checkpoint: &VerifiedCheckpoint) -> Result<()> {
        (*self).try_insert_checkpoint(checkpoint)
    }

    fn try_update_highest_synced_checkpoint(&self, checkpoint: &VerifiedCheckpoint) -> Result<()> {
        (*self).try_update_highest_synced_checkpoint(checkpoint)
    }

    fn try_update_highest_verified_checkpoint(
        &self,
        checkpoint: &VerifiedCheckpoint,
    ) -> Result<()> {
        (*self).try_update_highest_verified_checkpoint(checkpoint)
    }

    fn try_insert_checkpoint_contents(
        &self,
        checkpoint: &VerifiedCheckpoint,
        contents: VerifiedCheckpointContents,
    ) -> Result<()> {
        (*self).try_insert_checkpoint_contents(checkpoint, contents)
    }

    fn try_insert_committee(&self, new_committee: Committee) -> Result<()> {
        (*self).try_insert_committee(new_committee)
    }

    fn try_get_highest_executed_checkpoint_seq_number(
        &self,
    ) -> Result<Option<CheckpointSequenceNumber>> {
        (*self).try_get_highest_executed_checkpoint_seq_number()
    }

    fn try_insert_synced_checkpoints(
        &self,
        checkpoints: Vec<(VerifiedCheckpoint, VerifiedCheckpointContents)>,
    ) -> Result<()> {
        (*self).try_insert_synced_checkpoints(checkpoints)
    }
}

impl<T: WriteStore + ?Sized> WriteStore for Box<T> {
    fn try_insert_checkpoint(&self, checkpoint: &VerifiedCheckpoint) -> Result<()> {
        (**self).try_insert_checkpoint(checkpoint)
    }

    fn try_update_highest_synced_checkpoint(&self, checkpoint: &VerifiedCheckpoint) -> Result<()> {
        (**self).try_update_highest_synced_checkpoint(checkpoint)
    }

    fn try_update_highest_verified_checkpoint(
        &self,
        checkpoint: &VerifiedCheckpoint,
    ) -> Result<()> {
        (**self).try_update_highest_verified_checkpoint(checkpoint)
    }

    fn try_insert_checkpoint_contents(
        &self,
        checkpoint: &VerifiedCheckpoint,
        contents: VerifiedCheckpointContents,
    ) -> Result<()> {
        (**self).try_insert_checkpoint_contents(checkpoint, contents)
    }

    fn try_insert_committee(&self, new_committee: Committee) -> Result<()> {
        (**self).try_insert_committee(new_committee)
    }

    fn try_get_highest_executed_checkpoint_seq_number(
        &self,
    ) -> Result<Option<CheckpointSequenceNumber>> {
        (**self).try_get_highest_executed_checkpoint_seq_number()
    }

    fn try_insert_synced_checkpoints(
        &self,
        checkpoints: Vec<(VerifiedCheckpoint, VerifiedCheckpointContents)>,
    ) -> Result<()> {
        (**self).try_insert_synced_checkpoints(checkpoints)
    }
}

impl<T: WriteStore + ?Sized> WriteStore for Arc<T> {
    fn try_insert_checkpoint(&self, checkpoint: &VerifiedCheckpoint) -> Result<()> {
        (**self).try_insert_checkpoint(checkpoint)
    }

    fn try_update_highest_synced_checkpoint(&self, checkpoint: &VerifiedCheckpoint) -> Result<()> {
        (**self).try_update_highest_synced_checkpoint(checkpoint)
    }

    fn try_update_highest_verified_checkpoint(
        &self,
        checkpoint: &VerifiedCheckpoint,
    ) -> Result<()> {
        (**self).try_update_highest_verified_checkpoint(checkpoint)
    }

    fn try_insert_checkpoint_contents(
        &self,
        checkpoint: &VerifiedCheckpoint,
        contents: VerifiedCheckpointContents,
    ) -> Result<()> {
        (**self).try_insert_checkpoint_contents(checkpoint, contents)
    }

    fn try_insert_committee(&self, new_committee: Committee) -> Result<()> {
        (**self).try_insert_committee(new_committee)
    }

    fn try_get_highest_executed_checkpoint_seq_number(
        &self,
    ) -> Result<Option<CheckpointSequenceNumber>> {
        (**self).try_get_highest_executed_checkpoint_seq_number()
    }

    fn try_insert_synced_checkpoints(
        &self,
        checkpoints: Vec<(VerifiedCheckpoint, VerifiedCheckpointContents)>,
    ) -> Result<()> {
        (**self).try_insert_synced_checkpoints(checkpoints)
    }
}
