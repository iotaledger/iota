// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{io::SeekFrom, path::PathBuf};

use async_trait::async_trait;
use iota_types::messages_checkpoint::CheckpointSequenceNumber;
use serde_json::{Number, Value};
use tokio::{
    fs::{File, OpenOptions},
    io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt},
};

use crate::{IngestionError, IngestionResult, progress_store::ProgressStore};

/// Manages persistent progress information stored in a JSON file.
///
/// This struct encapsulates file operations for reading, writing, and
/// synchronizing progress data to disk. It uses asynchronous I/O provided by
/// [`tokio::fs`] for efficient operation within a Tokio runtime.
///
/// # Example
/// ```
/// use iota_data_ingestion_core::{FileProgressStore, ProgressStore};
///
/// #[tokio::main]
/// async fn main() {
///     let mut store = FileProgressStore::new("progress.json").await.unwrap();
///     store.save("task1".into(), 42).await.unwrap();
///     let checkpoint = store.load("task1".into()).await.unwrap();
///     assert_eq!(checkpoint, 42);
///     # tokio::fs::remove_file("progress.json").await.unwrap();
/// }
/// ```
pub struct FileProgressStore {
    /// The [`File`] handle used to interact with the progress file.
    file: File,
}

impl FileProgressStore {
    /// Creates a new `FileProgressStore` by opening or creating the file at the
    /// specified path.
    pub async fn new(path: impl Into<PathBuf>) -> IngestionResult<Self> {
        Self::open_or_create_file(path.into())
            .await
            .map(|file| Self { file })
    }

    /// Open or create the file at the specified path.
    async fn open_or_create_file(path: PathBuf) -> IngestionResult<File> {
        Ok(OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)
            .await?)
    }

    /// Returns an empty JSON object.
    fn empty_json_object() -> Value {
        Value::Object(serde_json::Map::new())
    }

    /// Checks if the file is empty.
    async fn is_file_empty(&self) -> IngestionResult<bool> {
        Ok(self.file.metadata().await.map(|m| m.len() == 0)?)
    }

    /// Reads the file content and parses it as a JSON [`Value`].
    ///
    /// - If the file is empty, this function *avoids reading the file* and
    ///   immediately returns an empty JSON object.
    /// - If the file is not empty, it reads the entire file content and parses
    ///   into a JSON [`Value`].
    /// - If JSON parsing fails (indicating a corrupted or invalid JSON file),
    ///   it also returns an empty JSON object. This ensures that the progress
    ///   store starts with a clean state in case of file corruption. Later, the
    ///   `ProgressStore::load` method will interpret this empty JSON object as
    ///   a default checkpoint sequence number of 0.
    async fn read_file_to_json_value(&mut self) -> IngestionResult<Value> {
        if self.is_file_empty().await? {
            return Ok(Self::empty_json_object());
        }
        // before reading seek to the start of the file
        self.file.seek(SeekFrom::Start(0)).await?;
        let mut buf = Vec::new();
        self.file.read_to_end(&mut buf).await?;
        Ok(serde_json::from_slice::<Value>(buf.as_slice())
            .inspect_err(|err| tracing::warn!("corrupted or invalid JSON file: {err}"))
            .unwrap_or_else(|_| Self::empty_json_object()))
    }

    /// Writes the given data to the file, overwriting any existing content.
    async fn write_to_file(&mut self, data: impl AsRef<[u8]>) -> IngestionResult<()> {
        // before writing seek to the start of the file
        self.file.seek(SeekFrom::Start(0)).await?;
        // clear the file content
        self.file.set_len(0).await?;
        Ok(self.file.write_all(data.as_ref()).await?)
    }
}

#[async_trait]
impl ProgressStore for FileProgressStore {
    type Error = IngestionError;

    async fn load(&mut self, task_name: String) -> Result<CheckpointSequenceNumber, Self::Error> {
        let content = self.read_file_to_json_value().await?;
        Ok(content
            .get(&task_name)
            .and_then(|v| v.as_u64())
            .unwrap_or_default())
    }
    async fn save(
        &mut self,
        task_name: String,
        checkpoint_number: CheckpointSequenceNumber,
    ) -> Result<(), Self::Error> {
        let mut content = self.read_file_to_json_value().await?;
        content[task_name] = Value::Number(Number::from(checkpoint_number));
        self.write_to_file(serde_json::to_string_pretty(&content)?)
            .await?;
        Ok(())
    }
}
