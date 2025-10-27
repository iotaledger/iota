// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use async_trait::async_trait;
use iota_data_ingestion_core::Worker;
use iota_types::full_checkpoint_content::CheckpointData;

use crate::{BigTableClient, KeyValueStoreWriter, TransactionData};

/// This worker implementation is responsible for processing checkpoints by
/// storing its data as Key-Value pairs. The Key-Value pairs are stored in a
/// BigTableDB.
pub struct KvWorker {
    pub client: BigTableClient,
}

#[async_trait]
impl Worker for KvWorker {
    type Message = ();
    type Error = anyhow::Error;

    async fn process_checkpoint(&self, checkpoint: Arc<CheckpointData>) -> anyhow::Result<()> {
        let mut client = self.client.clone();
        let mut objects = vec![];
        let mut transactions = Vec::with_capacity(checkpoint.transactions.len());
        for transaction in &checkpoint.transactions {
            for object in &transaction.output_objects {
                objects.push(object);
            }
            transactions.push(TransactionData::new(
                transaction,
                checkpoint.checkpoint_summary.sequence_number,
            ));
        }
        client.save_objects(&objects).await?;
        client.save_transactions(&transactions).await?;
        client.save_checkpoint(&checkpoint).await?;

        Ok(())
    }
}
