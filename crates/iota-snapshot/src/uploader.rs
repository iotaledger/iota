// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{num::NonZeroUsize, path::PathBuf, sync::Arc, time::Duration};

use anyhow::{Context, Result};
use bytes::Bytes;
use iota_config::object_storage_config::{ObjectStoreConfig, ObjectStoreType};
use iota_core::{checkpoints::CheckpointStore, state_snapshot::EpochSnapshotRequest};
use iota_sdk_types::CheckpointCommitment;
use iota_storage::{
    FileCompression,
    object_store::util::{
        EPOCH_METADATA_FILENAME, EpochMetadata, SUCCESS_MARKER, find_missing_epochs_dirs, put,
        run_manifest_update_loop,
    },
};
use iota_types::{digests::ChainIdentifier, messages_checkpoint::ECMHLiveObjectSetDigest};
use object_store::DynObjectStore;
use prometheus_filtered::{
    IntCounter, IntGauge, Registry, register_int_counter_with_registry,
    register_int_gauge_with_registry,
};
use tokio::sync::mpsc;
use tracing::{error, info};

use crate::writer::StateSnapshotWriterV1;

/// Default parallelism for uploading a snapshot's files to the remote store,
/// used when `state_snapshot_write_config.concurrency` is unset (`0`).
const DEFAULT_UPLOAD_CONCURRENCY: usize = 20;

pub struct StateSnapshotUploaderMetrics {
    pub first_missing_state_snapshot_epoch: IntGauge,
    pub state_snapshot_upload_err: IntCounter,
}

impl StateSnapshotUploaderMetrics {
    pub fn new(registry: &Registry) -> Arc<Self> {
        let this = Self {
            first_missing_state_snapshot_epoch: register_int_gauge_with_registry!(
                "first_missing_state_snapshot_epoch",
                "First epoch for which we have no state snapshot in remote store",
                registry
            )
            .unwrap(),
            state_snapshot_upload_err: register_int_counter_with_registry!(
                "state_snapshot_upload_err",
                "Track upload errors we can alert on",
                registry
            )
            .unwrap(),
        };
        Arc::new(this)
    }
}

/// StateSnapshotUploader is responsible for uploading state snapshots to remote
/// store.
pub struct StateSnapshotUploader {
    /// Source of per-epoch `EpochInfoV2` rows and epoch state commitments.
    checkpoint_store: Arc<CheckpointStore>,
    /// Directory path on local disk where state snapshots are staged for upload
    staging_path: PathBuf,
    /// Store on local disk where state snapshots are staged for upload
    staging_store: Arc<DynObjectStore>,
    /// Remote store i.e. S3, GCS, etc where state snapshots are uploaded to
    snapshot_store: Arc<DynObjectStore>,
    /// How often the first-missing-epoch metric is refreshed.
    interval: Duration,
    /// Parallelism for uploading a snapshot's files to the remote store.
    concurrency: NonZeroUsize,
    metrics: Arc<StateSnapshotUploaderMetrics>,
}

impl StateSnapshotUploader {
    pub fn new(
        staging_path: &std::path::Path,
        snapshot_store_config: ObjectStoreConfig,
        concurrency: usize,
        interval_s: u64,
        registry: &Registry,
        checkpoint_store: Arc<CheckpointStore>,
    ) -> Result<Arc<Self>> {
        let staging_store_config = ObjectStoreConfig {
            object_store: Some(ObjectStoreType::File),
            directory: Some(staging_path.to_path_buf()),
            ..Default::default()
        };
        Ok(Arc::new(StateSnapshotUploader {
            checkpoint_store,
            staging_path: staging_path.to_path_buf(),
            staging_store: staging_store_config.make()?,
            snapshot_store: snapshot_store_config.make()?,
            interval: Duration::from_secs(interval_s),
            concurrency: NonZeroUsize::new(concurrency)
                .unwrap_or(NonZeroUsize::new(DEFAULT_UPLOAD_CONCURRENCY).unwrap()),
            metrics: StateSnapshotUploaderMetrics::new(registry),
        }))
    }

    /// Starts the state snapshot uploader loop and manifest update loop.
    pub fn start(
        self: Arc<Self>,
        requests: mpsc::Receiver<EpochSnapshotRequest>,
    ) -> tokio::sync::broadcast::Sender<()> {
        let (kill_sender, _kill_receiver) = tokio::sync::broadcast::channel::<()>(1);
        tokio::task::spawn(Self::run_write_loop(
            self.clone(),
            requests,
            kill_sender.subscribe(),
        ));
        // On its own task: a remote listing that hangs must not delay taking
        // the read view an epoch boundary is waiting for.
        tokio::task::spawn(Self::run_missing_epochs_metric_loop(
            self.clone(),
            kill_sender.subscribe(),
        ));
        tokio::task::spawn(run_manifest_update_loop(
            self.snapshot_store.clone(),
            kill_sender.subscribe(),
        ));
        kill_sender
    }

    /// Writes and uploads the state snapshot of one epoch.
    ///
    /// The live-object scan reads through a view of the perpetual store taken
    /// at the epoch boundary, so it sees the state the epoch ended with while
    /// the node carries on executing. `request.view_taken` is signalled as
    /// soon as that view exists; until then the node is waiting.
    pub(crate) async fn write_state_snapshot(&self, request: EpochSnapshotRequest) -> Result<()> {
        let EpochSnapshotRequest {
            epoch,
            perpetual_tables,
            view_taken,
            // Held until this function returns, which is what frees the next
            // epoch boundary to hand its own snapshot over.
            writer_idle: _writer_idle,
        } = request;
        // Chain identifier = genesis checkpoint digest; tags each manifest.
        let chain_id = ChainIdentifier::from(
            *self
                .checkpoint_store
                .get_checkpoint_by_sequence_number(0)?
                .context("genesis checkpoint missing from checkpoint store")?
                .digest(),
        );
        info!("Starting state snapshot creation for epoch: {epoch}");
        let state_snapshot_writer = StateSnapshotWriterV1::new_from_store(
            &self.staging_path,
            &self.staging_store,
            &self.snapshot_store,
            self.checkpoint_store.clone(),
            chain_id,
            FileCompression::Zstd,
            self.concurrency,
        )
        .await?;
        let commitments = self
            .checkpoint_store
            .get_epoch_state_commitments(epoch)?
            .context("expected the last checkpoint of the epoch to carry end of epoch data")?;
        let CheckpointCommitment::EcmhLiveObjectSet { digest } = *commitments
            .last()
            .context("expected at least one epoch state commitment")?
        else {
            unimplemented!("a new CheckpointCommitment variant was added and must be handled")
        };
        state_snapshot_writer
            .write(
                epoch,
                perpetual_tables,
                ECMHLiveObjectSetDigest { digest },
                view_taken,
            )
            .await?;
        info!("State snapshot creation successful for epoch: {epoch}");

        let db_path = object_store::path::Path::from(format!("epoch_{epoch}"));
        // Records the on-chain end timestamp of this epoch (= timestamp of the
        // last checkpoint of the epoch) in each epoch bucket,
        // which will be read when updating the MANIFEST file.
        if let Some(checkpoint) = self.checkpoint_store.get_epoch_last_checkpoint(epoch)? {
            let metadata = EpochMetadata {
                epoch_end_timestamp_ms: checkpoint.timestamp_ms,
            };
            put(
                &self.snapshot_store,
                &db_path.child(EPOCH_METADATA_FILENAME),
                metadata.to_bytes()?,
            )
            .await?;
        } else {
            error!(
                "Could not determine epoch end timestamp for epoch {epoch}; skipping metadata write"
            );
        }
        // Drops marker in the output directory that upload completed successfully
        let success_marker = db_path.child(SUCCESS_MARKER);
        put(
            &self.snapshot_store,
            &success_marker,
            Bytes::from_static(b"success"),
        )
        .await?;
        info!("State snapshot completed for epoch: {epoch}");
        Ok(())
    }

    /// Writes the state snapshot of each epoch as the node hands it over.
    async fn run_write_loop(
        self: Arc<Self>,
        mut requests: mpsc::Receiver<EpochSnapshotRequest>,
        mut recv: tokio::sync::broadcast::Receiver<()>,
    ) -> Result<()> {
        info!("State snapshot writer loop started");
        loop {
            tokio::select! {
                request = requests.recv() => {
                    let Some(request) = request else { break };
                    let epoch = request.epoch;
                    if let Err(err) = self.write_state_snapshot(request).await {
                        self.metrics.state_snapshot_upload_err.inc();
                        error!("Failed to write the state snapshot for epoch {epoch}: {err:?}");
                    }
                },
                _ = recv.recv() => break,
            }
        }
        Ok(())
    }

    /// Keeps the first-missing-epoch metric current, for alerting on a node
    /// that has stopped publishing.
    async fn run_missing_epochs_metric_loop(
        self: Arc<Self>,
        mut recv: tokio::sync::broadcast::Receiver<()>,
    ) -> Result<()> {
        let mut interval = tokio::time::interval(self.interval);
        loop {
            tokio::select! {
                _now = interval.tick() => {
                    match self.get_missing_epochs().await {
                        Ok(epochs) => {
                            let first_missing_epoch = epochs.first().cloned().unwrap_or(0);
                            self.metrics
                                .first_missing_state_snapshot_epoch
                                .set(first_missing_epoch as i64);
                        }
                        Err(err) => {
                            error!("Failed to find missing state snapshot in remote store: {err:?}");
                        }
                    }
                },
                _ = recv.recv() => break,
            }
        }
        Ok(())
    }

    /// Finds missing epochs in the remote store.
    async fn get_missing_epochs(&self) -> Result<Vec<u64>> {
        let missing_epochs = find_missing_epochs_dirs(&self.snapshot_store, SUCCESS_MARKER).await?;
        Ok(missing_epochs.to_vec())
    }
}
