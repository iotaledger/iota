// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
//
use std::collections::HashMap;

use async_trait::async_trait;
use iota_data_ingestion_core::{DataIngestionMetrics, IndexerExecutor, ProgressStore};
use iota_types::base_types::Version;
use prometheus::Registry;
use tokio_util::sync::CancellationToken;

use crate::IndexerError;

pub(crate) struct ShimIndexerProgressStore {
    watermarks: HashMap<String, Version>,
}

impl ShimIndexerProgressStore {
    pub fn new(watermarks: Vec<(String, Version)>) -> Self {
        Self {
            watermarks: watermarks.into_iter().collect(),
        }
    }
}

#[async_trait]
impl ProgressStore for ShimIndexerProgressStore {
    type Error = IndexerError;

    async fn load(&mut self, task_name: String) -> Result<Version, Self::Error> {
        let err_msg = format!("missing watermark for {task_name}");
        Ok(*self.watermarks.get(&task_name).expect(&err_msg))
    }

    async fn save(&mut self, task_name: String, checkpoint: Version) -> Result<(), Self::Error> {
        self.watermarks.insert(task_name, checkpoint);
        Ok(())
    }
}

pub(crate) fn new_executor(
    task_name: String,
    watermark: Version,
    cancel: CancellationToken,
) -> IndexerExecutor<ShimIndexerProgressStore> {
    let progress_store = ShimIndexerProgressStore::new(vec![(task_name, watermark)]);
    IndexerExecutor::new(
        progress_store,
        1,
        DataIngestionMetrics::new(&Registry::new()),
        cancel.child_token(),
    )
}
