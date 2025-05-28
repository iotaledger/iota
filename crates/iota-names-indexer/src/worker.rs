// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{path::PathBuf, sync::Arc};

use anyhow::Result;
use async_trait::async_trait;
use iota_data_ingestion_core::{
    DataIngestionMetrics, FileProgressStore, IndexerExecutor, ReaderOptions, Worker, WorkerPool,
};
use iota_names::config::IotaNamesConfig;
use iota_types::{
    Identifier,
    effects::{TransactionEffects, TransactionEffectsAPI},
    event::Event,
    execution_status::ExecutionStatus,
    full_checkpoint_content::CheckpointData,
    transaction::{ProgrammableTransaction, TransactionData, TransactionKind},
};
use prometheus::Registry;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info};

use crate::IotaNamesMetrics;

pub(crate) async fn run_iota_names_reader(
    worker: IotaNamesWorker,
    node_url: &str,
    registry: &Registry,
    token: CancellationToken,
    concurrency: usize,
) -> anyhow::Result<()> {
    let progress_store = FileProgressStore::new("./progress_store").await?;

    let mut executor = IndexerExecutor::new(
        progress_store,
        1,
        DataIngestionMetrics::new(registry),
        token,
    );
    let worker_pool = WorkerPool::new(
        worker,
        "iota_names_reader".to_string(),
        concurrency,
        Default::default(),
    );
    executor.register(worker_pool).await?;

    info!("Connecting to node at {node_url}");
    executor
        .run(
            // path to a local directory where checkpoints are stored.
            PathBuf::from("./chk"),
            Some(format!("{node_url}/api/v1")),
            // optional remote store access options.
            vec![],
            ReaderOptions::default(),
        )
        .await?;
    Ok(())
}

pub(crate) struct IotaNamesWorker {
    config: IotaNamesConfig,
    metrics: Arc<IotaNamesMetrics>,
}

impl IotaNamesWorker {
    pub(crate) fn new(config: IotaNamesConfig, metrics: Arc<IotaNamesMetrics>) -> Self {
        Self { config, metrics }
    }

    fn process_event(&self, event: &Event) -> anyhow::Result<()> {
        if event.type_.address == self.config.package_address.into() {
            // TODO temporarily allowed until there are more even types
            #[allow(clippy::collapsible_if)]
            if event.type_.name == Identifier::new("IotaNamesRegistryEvent")? {
                // TODO: init from prometheus storage to not always start from 0
                self.metrics.total_name_records.add(1);
                // TODO: deserialize to get the name lengths
                // let register_event =
                //     bcs::from_bytes::<IotaNamesRegistryEvent>(&
                // event_bcs_bytes)?;
                // println!("Register event: {register_event:#?}");
            }
        }

        Ok(())
    }

    fn process_ptb(&self, _ptb: &ProgrammableTransaction) -> anyhow::Result<()> {
        let _module = Identifier::new("payment")?; // TODO: Make const
        let _function = Identifier::new("register")?;

        // if ptb.commands.iter().any(|cmd| {
        //     if let Command::MoveCall(call) = cmd {
        //         call.package == ObjectID::from(self.config.package_address)
        //             && call.module == module
        //             && call.function == function
        //     } else {
        //         false
        //     }
        // }) {}

        Ok(())
    }
}

#[async_trait]
impl Worker for IotaNamesWorker {
    type Message = ();
    type Error = anyhow::Error;

    async fn process_checkpoint(
        &self,
        checkpoint: Arc<CheckpointData>, // TODO change to &?
    ) -> Result<Self::Message, Self::Error> {
        debug!(
            "Processing checkpoint: {}",
            checkpoint.checkpoint_summary.sequence_number
        );

        for transaction in &checkpoint.transactions {
            let TransactionEffects::V1(effects) = &transaction.effects;

            if *effects.status() != ExecutionStatus::Success {
                continue;
            }

            if let Some(events) = &transaction.events {
                for event in events.data.iter() {
                    self.process_event(event)?;
                }
            }

            let TransactionData::V1(data) = &transaction.transaction.intent_message().value;

            if let TransactionKind::ProgrammableTransaction(ptb) = &data.kind {
                self.process_ptb(ptb)?;
            }
        }

        Ok(())
    }
}
