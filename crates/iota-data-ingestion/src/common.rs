// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::ops::Range;

use iota_rest_api::Client;
use iota_types::{committee::EpochId, messages_checkpoint::CheckpointSequenceNumber};

/// Get the current epoch.
pub async fn current_epoch(rest_client: &Client) -> anyhow::Result<EpochId> {
    let chk = rest_client.get_latest_checkpoint().await?;
    Ok(chk.epoch)
}

/// Get the range of [`CheckpointSequenceNumber`] from the first checkpoint of
/// the epoch containing the watermark up to but not including the watermark.
pub async fn checkpoint_sequence_number_range_to_watermark(
    rest_client: &Client,
    watermark: CheckpointSequenceNumber,
) -> anyhow::Result<Range<CheckpointSequenceNumber>> {
    let chk = rest_client.get_checkpoint_summary(watermark).await?;
    let chk_seq_num = epoch_first_checkpoint_sequence_number(rest_client, chk.epoch).await?;
    Ok(chk_seq_num..watermark)
}

/// Get the [`CheckpointSequenceNumber`] of the first checkpoint in the
/// specified epoch.
pub async fn epoch_first_checkpoint_sequence_number(
    rest_client: &Client,
    epoch: EpochId,
) -> anyhow::Result<CheckpointSequenceNumber> {
    let previous_epoch = epoch.saturating_sub(1);
    if epoch == 0 {
        return Ok(0);
    }
    let last_epoch_chk = rest_client
        .get_epoch_last_checkpoint(previous_epoch)
        .await?;
    Ok(last_epoch_chk.sequence_number + 1)
}
