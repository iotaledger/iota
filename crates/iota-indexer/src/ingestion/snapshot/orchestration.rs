// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use crate::PgIndexerStore;
use crate::config::SnapshotLagConfig;
use crate::ingestion::snapshot::persist::ObjectSnapshotWriter;
use crate::ingestion::snapshot::prepare::ObjectsSnapshotWorker;
use crate::types::IndexerResult;
use crate::{
    ingestion::primary::persist::TransactionObjectChangesToCommit, metrics::IndexerMetrics,
};
use iota_data_ingestion_core::WorkerPool;
use iota_metrics::get_metrics;
use tracing::info;

const OBJECT_SNAPSHOT_CHANNEL_CAPACITY: usize = 600;

pub async fn setup_snapshot(
    store: PgIndexerStore,
    metrics: IndexerMetrics,
    snapshot_config: SnapshotLagConfig,
    checkpoint_download_queue_size: usize,
) -> IndexerResult<(
    WorkerPool<ObjectsSnapshotWorker>,
    ObjectSnapshotWriter,
    iota_metrics::metered_channel::Receiver<(u64, TransactionObjectChangesToCommit)>,
)> {
    info!("Starting object snapshot handler...");

    let global_metrics = get_metrics().unwrap();
    let (sender, receiver) = iota_metrics::metered_channel::channel(
        OBJECT_SNAPSHOT_CHANNEL_CAPACITY,
        &global_metrics
            .channel_inflight
            .with_label_values(&["objects_snapshot_handler_checkpoint_data"]),
    );

    let worker_pool = WorkerPool::new(
        ObjectsSnapshotWorker::new(sender, metrics.clone()),
        "object_snapshot".to_string(),
        checkpoint_download_queue_size,
        Default::default(),
    );

    let writer = ObjectSnapshotWriter::new(store.clone(), metrics.clone(), snapshot_config);
    Ok((worker_pool, writer, receiver))
}
