// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use iota_data_ingestion_core::Worker;
use iota_rest_api::CheckpointData;
use iota_types::{base_types::ObjectID, transaction::TransactionDataAPI};
use move_core_types::identifier::IdentStr;
use tokio::sync::Mutex;

use crate::{FileType, handlers::AnalyticsHandler, tables::MoveCallEntry};

pub struct MoveCallHandler {
    state: Mutex<State>,
}

struct State {
    move_calls: Vec<MoveCallEntry>,
}

#[async_trait::async_trait]
impl Worker for MoveCallHandler {
    type Message = ();
    type Error = anyhow::Error;

    async fn process_checkpoint(
        &self,
        checkpoint_data: &CheckpointData,
    ) -> Result<Self::Message, Self::Error> {
        let CheckpointData {
            checkpoint_summary,
            transactions: checkpoint_transactions,
            ..
        } = checkpoint_data;
        let mut state = self.state.lock().await;
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
                &mut state,
            );
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl AnalyticsHandler<MoveCallEntry> for MoveCallHandler {
    async fn read(&self) -> Result<Vec<MoveCallEntry>> {
        let mut state = self.state.lock().await;
        let cloned = state.move_calls.clone();
        state.move_calls.clear();
        Ok(cloned)
    }

    fn file_type(&self) -> Result<FileType> {
        Ok(FileType::MoveCall)
    }

    fn name(&self) -> &str {
        "move_call"
    }
}

impl MoveCallHandler {
    pub fn new() -> Self {
        let state = State { move_calls: vec![] };
        Self {
            state: Mutex::new(state),
        }
    }
    fn process_move_calls(
        &self,
        epoch: u64,
        checkpoint: u64,
        timestamp_ms: u64,
        transaction_digest: String,
        move_calls: &[(&ObjectID, &IdentStr, &IdentStr)],
        state: &mut State,
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
            state.move_calls.push(entry);
        }
    }
}
