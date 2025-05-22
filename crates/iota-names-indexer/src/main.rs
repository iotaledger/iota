// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use iota_data_ingestion_core::{Worker, setup_single_workflow};
use iota_names::config::IotaNamesConfig;
use iota_types::{
    Identifier,
    base_types::ObjectID,
    effects::{TransactionEffects, TransactionEffectsAPI},
    execution_status::ExecutionStatus,
    full_checkpoint_content::CheckpointData,
    transaction::{Command, TransactionData, TransactionKind},
};

struct IotaNamesWorker;

#[async_trait]
impl Worker for IotaNamesWorker {
    type Message = ();
    type Error = anyhow::Error;

    async fn process_checkpoint(
        &self,
        checkpoint: Arc<CheckpointData>, // TODO change to &?
    ) -> Result<Self::Message, Self::Error> {
        let config = IotaNamesConfig::from_env().unwrap_or_default();

        let mut num_registrations = 0;
        for transaction in &checkpoint.transactions {
            let TransactionEffects::V1(effects) = &transaction.effects;

            if *effects.status() != ExecutionStatus::Success {
                continue;
            }

            if let Some(events) = &transaction.events {
                for event in events.data.iter() {
                    if event.package_id == ObjectID::from(config.package_address) {
                        println!(
                            "Event for tx {} in checkpoint {}: {event:#?}",
                            transaction.transaction.digest(),
                            checkpoint.checkpoint_summary.sequence_number
                        );
                    }
                }
            }
            let TransactionData::V1(data) = &transaction.transaction.intent_message().value;
            let module = Identifier::new("payment")?; // TODO: Make const
            let function = Identifier::new("register")?;

            match &data.kind {
                TransactionKind::ProgrammableTransaction(txn) => {
                    // println!("{txn:?}");
                    if txn.commands.iter().any(|cmd| {
                        if let Command::MoveCall(call) = cmd {
                            println!("{:?}", call.package);
                            call.package == ObjectID::from(config.package_address)
                                && call.module == module
                                && call.function == function
                        } else {
                            false
                        }
                    }) {
                        num_registrations += 1;
                    }
                }
                _ => (),
            }
        }
        if num_registrations != 0 {
            println!(
                "Registered {num_registrations} names in checkpoint {}",
                checkpoint.checkpoint_summary.sequence_number
            );
        }

        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let (executor, _term_sender) = setup_single_workflow(
        IotaNamesWorker,
        "http://127.0.0.1:9000/api/v1".to_string(), // fullnode REST API
        0,                                          // initial checkpoint number
        5,                                          // concurrency
        None,                                       // extra reader options
    )
    .await?;

    executor.await?;

    Ok(())
}
