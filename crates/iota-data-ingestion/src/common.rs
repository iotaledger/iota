// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::ops::Range;

use iota_grpc_client::Client;
use iota_types::{committee::EpochId, messages_checkpoint::CheckpointSequenceNumber};

/// Gets epoch id and its first checkpoint sequence number.
///
/// if `None`, returns the current epoch.
pub async fn epoch_info(
    client: &Client,
    epoch_id: Option<EpochId>,
) -> anyhow::Result<(EpochId, CheckpointSequenceNumber)> {
    let epoch = client
        .get_epoch(epoch_id, Some("epoch,first_checkpoint"))
        .await
        .map_err(anyhow::Error::new)?
        .into_inner();

    epoch
        .epoch_id()
        .and_then(|epoch_id| {
            epoch
                .first_checkpoint_sequence_number()
                .map(|ch| (epoch_id, ch))
        })
        .map_err(Into::into)
}

/// Get the range of [`CheckpointSequenceNumber`] from the first checkpoint of
/// the epoch containing the watermark up to but not including the watermark.
pub async fn checkpoint_sequence_number_range_to_watermark(
    client: &Client,
    watermark: CheckpointSequenceNumber,
) -> anyhow::Result<Range<CheckpointSequenceNumber>> {
    let chk = client
        .get_checkpoint_by_sequence_number(watermark, None, None, None)
        .await?
        .into_inner();

    let epoch_id = chk.summary()?.summary()?.epoch;
    let (_, epoch_first_checkpoint_seq_num) = epoch_info(client, Some(epoch_id)).await?;
    Ok(epoch_first_checkpoint_seq_num..watermark)
}
