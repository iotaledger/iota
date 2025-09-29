// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use iota_grpc_api::{CheckpointContent, NodeClient};
use iota_rest_api::CheckpointData;
use iota_types::messages_checkpoint::CheckpointSequenceNumber;
use tokio::sync::{mpsc, oneshot};
use tokio_stream::StreamExt;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, instrument, warn};

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
pub const CHECKPOINT_BUFFER_SIZE: usize = 1000;
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
        let (checkpoint_sender, checkpoint_receiver) = mpsc::channel(CHECKPOINT_BUFFER_SIZE);
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
        let (checkpoint_sender, checkpoint_receiver) = mpsc::channel(CHECKPOINT_BUFFER_SIZE);
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

    #[instrument(
        skip(self),
        fields(
            grpc_url = %self.grpc_url,
            starting_checkpoint = %self.starting_checkpoint_number
        ),
        name = "grpc_checkpoint_reader"
    )]
    pub async fn run(mut self) -> IngestionResult<()> {
        debug!(
            "Starting checkpoint reader from watermark {}",
            self.starting_checkpoint_number
        );

        const MAX_RETRIES: usize = 10;
        const INITIAL_BACKOFF_SECS: u64 = 1;
        const MAX_BACKOFF_SECS: u64 = 60;

        let mut retry_count = 0;
        let mut backoff_secs = INITIAL_BACKOFF_SECS;

        loop {
            // Check for exit signal first
            if let Ok(()) = self.exit_receiver.try_recv() {
                debug!("Received exit signal, shutting down reader");
                return Ok(());
            }

            // Try streaming
            match self.stream_with_retry().await {
                Ok(()) => {
                    debug!("Stream completed normally");
                    break;
                }
                Err(e) => {
                    if self.cancel.is_cancelled() {
                        debug!("Cancelled, stopping reader");
                        break;
                    }

                    retry_count += 1;
                    if retry_count > MAX_RETRIES {
                        return Err(IngestionError::Upstream(anyhow::anyhow!(
                            "Max retries ({}) exceeded. Last error: {}",
                            MAX_RETRIES,
                            e
                        )));
                    }

                    warn!(
                        "Stream failed (attempt {}/{}): {}, retrying in {} seconds...",
                        retry_count, MAX_RETRIES, e, backoff_secs
                    );

                    tokio::time::sleep(tokio::time::Duration::from_secs(backoff_secs)).await;

                    // Exponential backoff with cap
                    backoff_secs = std::cmp::min(backoff_secs * 2, MAX_BACKOFF_SECS);
                }
            }
        }

        Ok(())
    }

    #[instrument(skip(self), name = "stream_with_retry")]
    async fn stream_with_retry(&self) -> IngestionResult<()> {
        let client = NodeClient::connect(&self.grpc_url).await.map_err(|e| {
            IngestionError::Upstream(anyhow::anyhow!("Failed to connect to gRPC: {e}"))
        })?;

        // Get current watermark, either from provider or use starting checkpoint
        let current_watermark = if let Some(provider) = &self.watermark_provider {
            provider.get_current_watermark().await
        } else {
            self.starting_checkpoint_number
        };

        debug!(
            "Starting stream from watermark {} (WorkerPool mode)",
            current_watermark
        );

        let mut checkpoint_client = client.checkpoint_client().ok_or_else(|| {
            IngestionError::Upstream(anyhow::anyhow!("Failed to get checkpoint client"))
        })?;

        let mut stream = checkpoint_client
            .stream_checkpoints(Some(current_watermark), None, true)
            .await
            .map_err(|e| {
                IngestionError::Upstream(anyhow::anyhow!("Failed to stream checkpoints: {e}"))
            })?;

        let mut channel_closed = false;
        while let Some(result) = stream.next().await {
            if self.cancel.is_cancelled() {
                warn!("Cancelled, stopping stream");
                break;
            }

            let checkpoint_data: CheckpointData = match result {
                Ok(CheckpointContent::Data(grpc_data)) => {
                    // Convert from gRPC CheckpointData to iota_types CheckpointData
                    // This requires proper conversion logic
                    match grpc_data {
                        iota_grpc_types::CheckpointData::V1(v1_data) => {
                            iota_types::full_checkpoint_content::CheckpointData {
                                checkpoint_summary: v1_data.checkpoint_summary,
                                checkpoint_contents: v1_data.checkpoint_contents,
                                transactions: v1_data.transactions,
                            }
                        }
                    }
                }
                Ok(CheckpointContent::Summary(_)) => {
                    error!("Expected checkpoint data but got summary");
                    return Err(IngestionError::Upstream(anyhow::anyhow!(
                        "Expected checkpoint data but received summary"
                    )));
                }
                Err(e) => {
                    warn!("Stream error: {e}");
                    return Err(IngestionError::Upstream(anyhow::anyhow!(
                        "gRPC stream error: {e}"
                    )));
                }
            };

            if let Err(_e) = self.checkpoint_sender.send(Arc::new(checkpoint_data)).await {
                warn!("WorkerPool channel closed, stopping stream");
                channel_closed = true;
                break;
            }
        }

        if channel_closed {
            debug!("Stream stopped due to channel closure (receiver gone)");
            return Ok(());
        }

        warn!("Stream ended - this should only happen on cancellation or error");
        if !self.cancel.is_cancelled() {
            return Err(IngestionError::Upstream(anyhow::anyhow!(
                "gRPC stream ended unexpectedly"
            )));
        }
        Ok(())
    }
}
