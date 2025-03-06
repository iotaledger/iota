// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::HashMap,
    fmt::{Debug, Display},
};

use async_trait::async_trait;
use iota_types::messages_checkpoint::CheckpointSequenceNumber;
mod file;
pub use file::FileProgressStore;

use crate::{IngestionError, IngestionResult};

pub type ExecutorProgress = HashMap<String, CheckpointSequenceNumber>;

/// A trait defining the interface for persistent storage of checkpoint
/// progress.
///
/// This trait allows for loading and saving the progress of a task, represented
/// by a `task_name` & `CheckpointSequenceNumber` as key value pairs.
/// Implementations of this trait are responsible for persisting this progress
/// across restarts or failures.
#[async_trait]
pub trait ProgressStore: Send {
    type Error: Debug + Display;

    /// Loads the last saved checkpoint sequence number for a given task.
    async fn load(&mut self, task_name: String) -> Result<CheckpointSequenceNumber, Self::Error>;

    /// Saves the current checkpoint sequence number for a given task.
    async fn save(
        &mut self,
        task_name: String,
        checkpoint_number: CheckpointSequenceNumber,
    ) -> Result<(), Self::Error>;
}

pub struct ProgressStoreWrapper<P> {
    progress_store: P,
    pending_state: ExecutorProgress,
}

#[async_trait]
impl<P: ProgressStore> ProgressStore for ProgressStoreWrapper<P> {
    type Error = IngestionError;

    async fn load(&mut self, task_name: String) -> Result<CheckpointSequenceNumber, Self::Error> {
        let watermark = self
            .progress_store
            .load(task_name.clone())
            .await
            .map_err(|err| IngestionError::ProgressStore(err.to_string()))?;
        self.pending_state.insert(task_name, watermark);
        Ok(watermark)
    }

    async fn save(
        &mut self,
        task_name: String,
        checkpoint_number: CheckpointSequenceNumber,
    ) -> Result<(), Self::Error> {
        self.progress_store
            .save(task_name.clone(), checkpoint_number)
            .await
            .map_err(|err| IngestionError::ProgressStore(err.to_string()))?;
        self.pending_state.insert(task_name, checkpoint_number);
        Ok(())
    }
}

impl<P: ProgressStore> ProgressStoreWrapper<P> {
    pub fn new(progress_store: P) -> Self {
        Self {
            progress_store,
            pending_state: HashMap::new(),
        }
    }

    pub fn min_watermark(&self) -> IngestionResult<CheckpointSequenceNumber> {
        self.pending_state
            .values()
            .min()
            .cloned()
            .ok_or(IngestionError::EmptyWorkerPool)
    }

    pub fn stats(&self) -> ExecutorProgress {
        self.pending_state.clone()
    }
}

/// A simple, in-memory progress store primarily used for unit testing.
///
/// # Note
///
/// Provides `save` and `load`, but the `save` is not persistent.
///
/// # Example
/// ```rust
/// use iota_data_ingestion_core::{ProgressStore, ShimProgressStore};
///
/// #[tokio::main]
/// async fn main() {
///     let mut store = ShimProgressStore(10);
///     // will not save the data.
///     store.save("task1".into(), 42).await.unwrap();
///     // ignores the task_name argument.
///     let checkpoint = store.load("task1".into()).await.unwrap();
///     assert_eq!(checkpoint, 10);
///     let checkpoint = store.load("task2".into()).await.unwrap();
///     assert_eq!(checkpoint, 10);
/// }
/// ```
pub struct ShimProgressStore(pub u64);

#[async_trait]
impl ProgressStore for ShimProgressStore {
    type Error = IngestionError;

    async fn load(&mut self, _: String) -> Result<CheckpointSequenceNumber, Self::Error> {
        Ok(self.0)
    }
    async fn save(&mut self, _: String, _: CheckpointSequenceNumber) -> Result<(), Self::Error> {
        Ok(())
    }
}
