// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use futures::future::OptionFuture;
use iota_rest_api::Client;
use iota_types::{
    committee::EpochId,
    messages_checkpoint::{CertifiedCheckpointSummary, CheckpointSequenceNumber},
};

/// Represents the current state at the network tip.
#[derive(Debug)]
pub enum NetworkTipState {
    /// Watermark is in the current epoch.
    CurrentEpoch { epoch: EpochId },
    /// Epoch has changed since the last watermark.
    EpochChanged {
        epoch: EpochId,
        first_chk_seq_num_of_epoch: CheckpointSequenceNumber,
    },
}

impl NetworkTipState {
    /// Returns the checkpoint number to use as the new watermark when an epoch
    /// change occurs.
    ///
    /// When the epoch changes, we need to reset our watermark to the first
    /// checkpoint of the new epoch. This method determines if such a reset
    /// is needed and returns the appropriate checkpoint number.
    pub fn should_update_watermark(&self) -> Option<CheckpointSequenceNumber> {
        match self {
            Self::EpochChanged {
                first_chk_seq_num_of_epoch,
                ..
            } => Some(*first_chk_seq_num_of_epoch),
            Self::CurrentEpoch { .. } => None,
        }
    }
}

/// Tracks the current state of checkpoints at the tip of the network.
pub struct ChainTipWatermarkTracker {
    client: Client,
    last_worker_watermark: Option<CheckpointSequenceNumber>,
}

impl ChainTipWatermarkTracker {
    /// Creates a new chain tip tracker.
    pub fn new(
        node_rest_api_url: &str,
        last_worker_watermark: Option<CheckpointSequenceNumber>,
    ) -> Self {
        Self {
            client: Client::new(node_rest_api_url),
            last_worker_watermark,
        }
    }

    /// Resolves the current state at the network tip.
    pub async fn resolve_network_tip_state(&self) -> anyhow::Result<NetworkTipState> {
        let latest_checkpoint = self.client.get_latest_checkpoint().await?;

        let latest_watermark_checkpoint: OptionFuture<_> = self
            .last_worker_watermark
            .map(|chk_seq_num| self.client.get_checkpoint_summary(chk_seq_num))
            .into();

        let status = match latest_watermark_checkpoint.await {
            Some(Ok(summary)) if summary.epoch != latest_checkpoint.epoch => {
                let watermark = self
                    .find_first_checkpoint_of_current_epoch(&latest_checkpoint)
                    .await?;
                NetworkTipState::EpochChanged {
                    epoch: latest_checkpoint.epoch,
                    first_chk_seq_num_of_epoch: watermark,
                }
            }
            Some(Ok(summary)) => NetworkTipState::CurrentEpoch {
                epoch: summary.epoch,
            },
            None | Some(Err(_)) => {
                let watermark = self
                    .find_first_checkpoint_of_current_epoch(&latest_checkpoint)
                    .await?;
                NetworkTipState::EpochChanged {
                    epoch: latest_checkpoint.epoch,
                    first_chk_seq_num_of_epoch: watermark,
                }
            }
        };

        Ok(status)
    }

    /// Finds the first checkpoint sequence number of a given epoch.
    ///
    /// This function starts from the latest checkpoint and iterates backwards,
    /// querying the rest api client for checkpoint summaries until it
    /// finds the last checkpoint of the previous epoch. The next checkpoint's
    /// sequence number is then returned as the first checkpoint of the target
    /// epoch.
    async fn find_first_checkpoint_of_current_epoch(
        &self,
        latest_checkpoint: &CertifiedCheckpointSummary,
    ) -> anyhow::Result<CheckpointSequenceNumber> {
        let target_epoch = latest_checkpoint.epoch;
        // start from the latest checkpoint and go backwards.
        let mut current = latest_checkpoint.sequence_number;
        if current == 0 {
            return Ok(0);
        }
        // search backwards to find where the epoch changes.
        while current > 0 {
            // check the previous checkpoint.
            match self.client.get_checkpoint_summary(current).await {
                Ok(summary) => {
                    if summary.is_last_checkpoint_of_epoch() {
                        let first_checkpoint_of_target_epoch = summary.sequence_number + 1;
                        tracing::info!(
                            "Found first checkpoint of epoch {target_epoch}: checkpoint {first_checkpoint_of_target_epoch}",
                        );
                        return Ok(first_checkpoint_of_target_epoch);
                    }
                }
                Err(e) => {
                    tracing::warn!("Error getting checkpoint {current}: {e}");
                    continue;
                }
            }
            current -= 1;
        }
        // if we reached checkpoint 0, it must be the first one.
        tracing::info!("Epoch {} starts at checkpoint 0", target_epoch);
        Ok(0)
    }
}
