// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

use super::{
    address_metrics_processor::AddressMetricsProcessor,
    move_call_metrics_processor::MoveCallMetricsProcessor,
    network_metrics_processor::NetworkMetricsProcessor,
};
use crate::{metrics::IndexerMetrics, store::IndexerAnalyticalStore};

pub struct ProcessorOrchestrator<S> {
    store: S,
    metrics: IndexerMetrics,
    cancel: CancellationToken,
}

impl<S> ProcessorOrchestrator<S>
where
    S: IndexerAnalyticalStore + Clone + Send + Sync + 'static,
{
    pub fn new(store: S, metrics: IndexerMetrics, cancel: CancellationToken) -> Self {
        Self {
            store,
            metrics,
            cancel,
        }
    }

    pub async fn run_forever(&mut self) {
        info!("Processor orchestrator started...");
        let mut tasks = JoinSet::new();

        let network_metrics_processor =
            NetworkMetricsProcessor::new(self.store.clone(), self.metrics.clone());

        tasks.spawn(async move {
            loop {
                let network_metrics_res = network_metrics_processor.start().await;
                if let Err(e) = network_metrics_res {
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    error!(
                        "indexer network metrics processor failed with error {e:?}, retrying in 5s..."
                    );
                }
            }
        });

        let addr_metrics_processor =
            AddressMetricsProcessor::new(self.store.clone(), self.metrics.clone());

        tasks.spawn(async move {
            loop {
                let addr_metrics_res = addr_metrics_processor.start().await;
                if let Err(e) = addr_metrics_res {
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    error!(
                        "indexer address metrics processor failed with error {e:?}, retrying in 5s..."
                    );
                }
            }
        });

        let move_call_metrics_processor =
            MoveCallMetricsProcessor::new(self.store.clone(), self.metrics.clone());

        tasks.spawn(async move {
            loop {
                let move_call_metrics_res = move_call_metrics_processor.start().await;
                if let Err(e) = move_call_metrics_res {
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    error!(
                        "indexer move call metrics processor failed with error {e:?}, retrying in 5s..."
                    );
                }
            }
        });

        tokio::select! {
            _ = self.cancel.cancelled() => {
                info!("Processor orchestrator shutting down...");
                // Aborting mid-batch is safe, each batch commits in one transaction,
                // so an interrupted one rolls back, and the processors resume from the
                // last committed cursor on the next start.
                tasks.shutdown().await;
            }
            _ = async {
                while let Some(res) = tasks.join_next().await {
                    res.expect("processor orchestrator should not run into errors.");
                }
            } => {}
        }
    }
}
