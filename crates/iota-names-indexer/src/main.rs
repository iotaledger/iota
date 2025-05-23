// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
    sync::Arc,
};

use anyhow::Result;
use async_trait::async_trait;
use iota_data_ingestion_core::{
    DataIngestionMetrics, FileProgressStore, IndexerExecutor, ReaderOptions, Worker, WorkerPool,
};
use iota_names::config::IotaNamesConfig;
use iota_types::{
    Identifier,
    base_types::ObjectID,
    effects::{TransactionEffects, TransactionEffectsAPI},
    execution_status::ExecutionStatus,
    full_checkpoint_content::CheckpointData,
    transaction::{Command, TransactionData, TransactionKind},
};
use tokio_util::sync::CancellationToken;

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
    let cancel_token = CancellationToken::new();
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 9184);
    let registry = iota_metrics::start_prometheus_server(addr).default_registry();
    iota_metrics::init_metrics(&registry);

    let backfill_progress_file_path = "./backfill_progress".to_string();
    let progress_store = FileProgressStore::new(PathBuf::from(backfill_progress_file_path)).await?;

    let metrics = DataIngestionMetrics::new(&registry);
    let mut executor = IndexerExecutor::new(progress_store, 1, metrics, cancel_token);

    let worker_pool = WorkerPool::new(
        IotaNamesWorker,
        "iota_names_reader".to_string(),
        1,
        Default::default(),
    );
    // register the worker pool to the executor.
    executor.register(worker_pool).await.unwrap();
    // run the ingestion pipeline.
    executor
        .run(
            PathBuf::from("./chk".to_string()), /* path to a local directory where checkpoints
                                                 * are stored. */
            Some("http://localhost:9000/api/v1".to_string()),
            vec![],                   // optional remote store access options.
            ReaderOptions::default(), // remote_read_batch_size.
        )
        .await
        .unwrap();

    Ok(())
}
