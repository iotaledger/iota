// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{path::PathBuf, str::FromStr, sync::Arc};

use anyhow::Result;
use async_trait::async_trait;
use iota_data_ingestion_core::{
    DataIngestionMetrics, FileProgressStore, IndexerExecutor, ReaderOptions, Worker, WorkerPool,
};
use iota_json::IotaJsonValue;
use iota_names::config::IotaNamesConfig;
use iota_types::{
    Identifier, TypeTag,
    balance::Balance,
    effects::{TransactionEffects, TransactionEffectsAPI},
    execution_status::ExecutionStatus,
    full_checkpoint_content::CheckpointData,
    transaction::{ProgrammableTransaction, TransactionData, TransactionKind},
};
use move_core_types::{
    annotated_value::{MoveFieldLayout, MoveStructLayout, MoveTypeLayout},
    language_storage::StructTag,
};
use prometheus::Registry;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use crate::{IotaNamesMetrics, events::IotaNamesEvent};

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

    fn process_event(&self, event: IotaNamesEvent) -> anyhow::Result<()> {
        match event {
            IotaNamesEvent::IotaNamesRegistry(_event) => {
                self.metrics.total_name_records.add(1);
            }
            IotaNamesEvent::IotaNamesReverseRegistry(_event) => (),
            IotaNamesEvent::AuctionStarted(_event) => (),
            IotaNamesEvent::AuctionBid(_event) => (),
            IotaNamesEvent::AuctionExtended(_event) => (),
            IotaNamesEvent::AuctionFinalized(_event) => (),
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

        let config_type = StructTag::from_str(&format!(
            "{}::iota_names::BalanceKey<iota::iota::IOTA>",
            self.config.package_address,
        ))?;
        let layout = MoveTypeLayout::Struct(Box::new(MoveStructLayout {
            type_: config_type.clone(),
            fields: vec![MoveFieldLayout::new(
                Identifier::from_str("dummy_field")?,
                MoveTypeLayout::Bool,
            )],
        }));
        let balance_object_id = iota_types::dynamic_field::derive_dynamic_field_id(
            self.config.object_id,
            &TypeTag::Struct(Box::new(config_type)),
            &IotaJsonValue::new(serde_json::json!({ "dummy_field": false }))?
                .to_bcs_bytes(&layout)?,
        )?;

        for object in checkpoint.latest_live_output_objects() {
            if object.id() == balance_object_id {
                let balance: Balance = object.to_rust().expect("invalid balance object");
                self.metrics.iota_names_balance.set(balance.value() as _);
                break;
            }
        }

        for transaction in &checkpoint.transactions {
            let TransactionEffects::V1(effects) = &transaction.effects;

            if *effects.status() != ExecutionStatus::Success {
                continue;
            }

            if let Some(events) = &transaction.events {
                for event in events.data.iter() {
                    match IotaNamesEvent::try_from_event(event, &self.config) {
                        Ok(Some(event)) => self.process_event(event)?,
                        Err(e) => warn!("parsing event failed: {e}"),
                        _ => {}
                    }
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
