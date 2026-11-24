// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::ops::RangeInclusive;

use async_trait::async_trait;
use futures::{
    StreamExt, TryStreamExt,
    stream::{self, BoxStream},
};
use iota_types::{
    full_checkpoint_content::CheckpointData, messages_checkpoint::CheckpointSequenceNumber,
};

use crate::{
    IngestionError, IngestionResult,
    backfill::{order::IngestionOrder, source::BackfillSource},
    history::reader::HistoricalReader,
};

/// The historical store serves a backfill one checkpoint file at a time.
#[async_trait]
impl BackfillSource for HistoricalReader {
    async fn checkpoints(
        &self,
        range: RangeInclusive<CheckpointSequenceNumber>,
        order: IngestionOrder,
    ) -> IngestionResult<BoxStream<'_, IngestionResult<CheckpointData>>> {
        self.sync_manifest_once().await?;
        let manifest = self.get_manifest().await;
        let stored_end = manifest.next_checkpoint_seq_num();
        if *range.end() >= stored_end {
            return Err(IngestionError::HistoryRead(format!(
                "checkpoints {range:?} are not all stored, the historical store holds 0..{stored_end}"
            )));
        }

        let mut files = self.verify_and_get_manifest_files(manifest)?;
        files.retain(|file| {
            let stored = &file.checkpoint_seq_range;
            stored.start <= *range.end() && *range.start() < stored.end
        });
        if order.is_descending() {
            files.reverse();
        }

        // The stream is lazy. The next file is downloaded only after every checkpoint of the
        // current one was pulled, so one file is in memory at a time.
        Ok(stream::iter(files)
            .then(move |file| {
                let range = range.clone();
                async move {
                    let mut checkpoints = self
                        .iter_for_file(file.file_path())
                        .await?
                        .filter(|checkpoint| {
                            range.contains(&checkpoint.checkpoint_summary.sequence_number)
                        })
                        .collect::<Vec<CheckpointData>>();
                    if order.is_descending() {
                        checkpoints.reverse();
                    }
                    Ok::<_, IngestionError>(stream::iter(checkpoints).map(Ok))
                }
            })
            .try_flatten()
            .boxed())
    }
}
