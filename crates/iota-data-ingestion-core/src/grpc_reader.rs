// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use anyhow;
use iota_grpc_api::client::GrpcNodeClient;
use iota_rest_api::CheckpointData;
use iota_types::messages_checkpoint::CheckpointSequenceNumber;
use tokio::sync::{mpsc, oneshot};
use tokio_stream::StreamExt;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::{IngestionError, IngestionResult};

/// gRPC checkpoint reader that streams checkpoints from a gRPC endpoint.
pub struct GrpcCheckpointReader {
    grpc_url: String,
    starting_checkpoint_number: CheckpointSequenceNumber,
    checkpoint_sender: mpsc::Sender<Arc<CheckpointData>>,
    exit_receiver: oneshot::Receiver<()>,
    cancel: CancellationToken,
    watermark_provider: Option<Box<dyn WatermarkProvider + Send + Sync>>,
}

/// Trait for providing the current watermark dynamically.
#[async_trait::async_trait]
pub trait WatermarkProvider {
    /// Get the current watermark (starting checkpoint number).
    async fn get_current_watermark(&self) -> CheckpointSequenceNumber;
}

impl GrpcCheckpointReader {
    pub fn initialize(
        grpc_url: String,
        starting_checkpoint_number: CheckpointSequenceNumber,
        cancel: CancellationToken,
    ) -> (
        Self,
        mpsc::Receiver<Arc<CheckpointData>>,
        oneshot::Sender<()>,
    ) {
        let (checkpoint_sender, checkpoint_receiver) = mpsc::channel(1000);
        let (exit_sender, exit_receiver) = oneshot::channel();

        let reader = Self {
            grpc_url,
            starting_checkpoint_number,
            checkpoint_sender,
            exit_receiver,
            cancel,
            watermark_provider: None,
        };

        (reader, checkpoint_receiver, exit_sender)
    }

    pub fn initialize_with_watermark_provider(
        grpc_url: String,
        starting_checkpoint_number: CheckpointSequenceNumber,
        cancel: CancellationToken,
        watermark_provider: Box<dyn WatermarkProvider + Send + Sync>,
    ) -> (
        Self,
        mpsc::Receiver<Arc<CheckpointData>>,
        oneshot::Sender<()>,
    ) {
        let (checkpoint_sender, checkpoint_receiver) = mpsc::channel(1000);
        let (exit_sender, exit_receiver) = oneshot::channel();

        let reader = Self {
            grpc_url,
            starting_checkpoint_number,
            checkpoint_sender,
            exit_receiver,
            cancel,
            watermark_provider: Some(watermark_provider),
        };

        (reader, checkpoint_receiver, exit_sender)
    }

    pub async fn run(mut self) -> IngestionResult<()> {
        info!(
            "[gRPC] Starting checkpoint reader from watermark {}",
            self.starting_checkpoint_number
        );

        loop {
            // Check for exit signal first
            if let Ok(()) = self.exit_receiver.try_recv() {
                info!("[gRPC] Received exit signal, shutting down reader");
                return Ok(());
            }

            // Try streaming
            match self.stream_with_retry().await {
                Ok(()) => {
                    info!("[gRPC] Stream completed normally");
                    break;
                }
                Err(e) => {
                    if self.cancel.is_cancelled() {
                        info!("[gRPC] Cancelled, stopping reader");
                        break;
                    }
                    warn!("[gRPC] Stream failed: {}, retrying in 1 second...", e);
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                }
            }
        }

        Ok(())
    }

    async fn stream_with_retry(&self) -> IngestionResult<()> {
        let mut client = GrpcNodeClient::connect(&self.grpc_url).await.map_err(|e| {
            IngestionError::Upstream(anyhow::anyhow!("Failed to connect to gRPC: {e}"))
        })?;

        // Get current watermark, either from provider or use starting checkpoint
        let current_watermark = if let Some(provider) = &self.watermark_provider {
            provider.get_current_watermark().await
        } else {
            self.starting_checkpoint_number
        };

        info!(
            "[gRPC] Starting stream from watermark {} (WorkerPool mode)",
            current_watermark
        );

        let mut stream = client
            .stream_checkpoints(Some(current_watermark), None, Some(true))
            .await
            .map_err(|e| {
                IngestionError::Upstream(anyhow::anyhow!("Failed to stream checkpoints: {e}"))
            })?;

        while let Some(result) = stream.next().await {
            if self.cancel.is_cancelled() {
                warn!("[gRPC] Cancelled, stopping stream");
                break;
            }

            let cp = match result {
                Ok(cp) => cp,
                Err(e) => {
                    warn!("[gRPC] Stream error: {e}");
                    return Err(IngestionError::Upstream(anyhow::anyhow!(
                        "gRPC stream error: {e}"
                    )));
                }
            };

            let checkpoint_data: CheckpointData = match GrpcNodeClient::deserialize_checkpoint(&cp)
            {
                Ok(iota_grpc_api::client::CheckpointContent::Data(data)) => data,
                Ok(iota_grpc_api::client::CheckpointContent::Summary(_)) => {
                    warn!("[gRPC] Expected checkpoint data but received summary, skipping");
                    continue;
                }
                Err(e) => {
                    warn!("[gRPC] BCS decode error: {e}");
                    continue;
                }
            };

            if let Err(_e) = self.checkpoint_sender.send(Arc::new(checkpoint_data)).await {
                warn!("[gRPC] WorkerPool channel closed");
                break;
            }
        }

        warn!("[gRPC] Stream ended - this should only happen on cancellation or error");
        if !self.cancel.is_cancelled() {
            return Err(IngestionError::Upstream(anyhow::anyhow!(
                "gRPC stream ended unexpectedly"
            )));
        }
        Ok(())
    }
}
