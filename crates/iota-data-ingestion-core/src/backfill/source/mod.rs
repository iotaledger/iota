// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod historical;

use std::{num::NonZeroUsize, ops::RangeInclusive};

use async_trait::async_trait;
use futures::stream::BoxStream;
use iota_types::{
    full_checkpoint_content::CheckpointData, messages_checkpoint::CheckpointSequenceNumber,
};

use crate::{
    IngestionError, IngestionResult, backfill::order::IngestionOrder,
    history::reader::HistoricalReader, reader::v2::RemoteUrl,
};

/// A store that a [`BackfillExecutor`](crate::BackfillExecutor) reads a bounded checkpoint
/// range from.
#[async_trait]
pub(crate) trait BackfillSource: Send + Sync {
    /// Streams the checkpoints of `range` in `order`.
    async fn checkpoints(
        &self,
        range: RangeInclusive<CheckpointSequenceNumber>,
        order: IngestionOrder,
    ) -> IngestionResult<BoxStream<'_, IngestionResult<CheckpointData>>>;
}

/// Creates the store a backfill reads from.
pub(crate) fn backfill_source(
    remote_url: RemoteUrl,
    download_concurrency: usize,
) -> IngestionResult<Box<dyn BackfillSource>> {
    match remote_url {
        // The live store only holds the newest checkpoints, which a backfill does not need.
        RemoteUrl::HybridHistoricalStore { historical_url, .. } => {
            let download_concurrency =
                NonZeroUsize::new(download_concurrency).unwrap_or(NonZeroUsize::MIN);
            Ok(Box::new(HistoricalReader::from_url(
                historical_url,
                download_concurrency,
            )?))
        }
        RemoteUrl::Fullnode(_) => Err(IngestionError::Unsupported(
            "backfill cannot read checkpoints from a fullnode yet".into(),
        )),
    }
}
