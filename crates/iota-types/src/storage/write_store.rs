// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use super::error::Result;
use crate::{
    committee::Committee,
    messages_checkpoint::{VerifiedCheckpoint, VerifiedCheckpointContents},
    storage::ReadStore,
};

#[::iota_macros::extend_trait_with_non_fallible("write to store failed")]
pub trait WriteStore: ReadStore {
    fn try_insert_checkpoint(&self, checkpoint: &VerifiedCheckpoint) -> Result<()>;
    fn try_update_highest_synced_checkpoint(&self, checkpoint: &VerifiedCheckpoint) -> Result<()>;
    fn try_update_highest_verified_checkpoint(&self, checkpoint: &VerifiedCheckpoint)
    -> Result<()>;
    fn try_insert_checkpoint_contents(
        &self,
        checkpoint: &VerifiedCheckpoint,
        contents: VerifiedCheckpointContents,
    ) -> Result<()>;

    fn try_insert_committee(&self, new_committee: Committee) -> Result<()>;
}

#[::iota_macros::extend_impl_with_non_fallible("write to store failed")]
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
}

#[::iota_macros::extend_impl_with_non_fallible("write to store failed")]
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
}

#[::iota_macros::extend_impl_with_non_fallible("write to store failed")]
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
}
