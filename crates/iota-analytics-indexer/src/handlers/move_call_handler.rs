// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use iota_indexer::framework::Handler;
use iota_rest_api::CheckpointData;
use iota_types::{base_types::ObjectID, transaction::TransactionDataAPI};
use move_core_types::identifier::IdentStr;

use crate::{handlers::AnalyticsHandler, tables::MoveCallEntry, FileType};

pub struct MoveCallHandler {
    move_calls: Vec<MoveCallEntry>,
}

#[async_trait::async_trait]
impl Handler for MoveCallHandler {
    fn name(&self) -> &str {
        "move_call"
    }
    async fn process_checkpoint(&mut self, checkpoint_data: &CheckpointData) -> Result<()> {
        let CheckpointData {
            checkpoint_summary,
            transactions: checkpoint_transactions,
            ..
        } = checkpoint_data;
        for checkpoint_transaction in checkpoint_transactions {
            let move_calls = checkpoint_transaction
                .transaction
                .transaction_data()
                .move_calls();
            self.process_move_calls(
                checkpoint_summary.epoch,
                checkpoint_summary.sequence_number,
                checkpoint_summary.timestamp_ms,
                checkpoint_transaction.transaction.digest().base58_encode(),
                &move_calls,
            );
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl AnalyticsHandler<MoveCallEntry> for MoveCallHandler {
    fn read(&mut self) -> Result<Vec<MoveCallEntry>> {
        let cloned = self.move_calls.clone();
        self.move_calls.clear();
        Ok(cloned)
    }

    fn file_type(&self) -> Result<FileType> {
        Ok(FileType::MoveCall)
    }
}

impl MoveCallHandler {
    pub fn new() -> Self {
        MoveCallHandler { move_calls: vec![] }
    }
    fn process_move_calls(
        &mut self,
        epoch: u64,
        checkpoint: u64,
        timestamp_ms: u64,
        transaction_digest: String,
        move_calls: &[(&ObjectID, &IdentStr, &IdentStr)],
    ) {
        for (package, module, function) in move_calls.iter() {
            let entry = MoveCallEntry {
                transaction_digest: transaction_digest.clone(),
                checkpoint,
                epoch,
                timestamp_ms,
                package: package.to_string(),
                module: module.to_string(),
                function: function.to_string(),
            };
            self.move_calls.push(entry);
        }
    }
}
