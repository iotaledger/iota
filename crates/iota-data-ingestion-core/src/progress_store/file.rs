// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::path::PathBuf;

use async_trait::async_trait;
use iota_types::messages_checkpoint::CheckpointSequenceNumber;
use serde_json::{Number, Value};

use crate::{IngestionError, progress_store::ProgressStore};

pub struct FileProgressStore {
    path: PathBuf,
}

impl FileProgressStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

#[async_trait]
impl ProgressStore for FileProgressStore {
    type Error = IngestionError;

    async fn load(&mut self, task_name: String) -> Result<CheckpointSequenceNumber, Self::Error> {
        let content: Value = serde_json::from_slice(&std::fs::read(self.path.clone())?)?;
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
        let mut content: Value = serde_json::from_slice(&std::fs::read(self.path.clone())?)?;
        content[task_name] = Value::Number(Number::from(checkpoint_number));
        std::fs::write(self.path.clone(), serde_json::to_string_pretty(&content)?)?;
        Ok(())
    }
}
