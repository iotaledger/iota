// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use anyhow::{Result, anyhow};
use futures::{StreamExt, TryStreamExt};
use iota_data_ingestion_core::{create_remote_store_client, reader::fetch_from_object_store};
use iota_types::{
    messages_checkpoint::{CheckpointSequenceNumber, VerifiedCheckpoint},
    storage::WriteStore,
};

pub(crate) async fn read_summaries_for_list_no_verify<S>(
    ingestion_url: String,
    concurrency: usize,
    store: S,
    checkpoints: Vec<CheckpointSequenceNumber>,
    checkpoint_counter: Arc<AtomicU64>,
) -> Result<()>
where
    S: WriteStore + Clone,
{
    let client = create_remote_store_client(ingestion_url, vec![], 60)?;
    futures::stream::iter(checkpoints)
        .map(|sq| fetch_from_object_store(&client, sq))
        .buffer_unordered(concurrency)
        .map_err(anyhow::Error::from)
        .try_for_each(|checkpoint| {
            let result = store
                .try_insert_checkpoint(&VerifiedCheckpoint::new_unchecked(
                    checkpoint.0.checkpoint_summary.clone(),
                ))
                .map_err(|e| anyhow!("Failed to insert checkpoint: {e}"));
            checkpoint_counter.fetch_add(1, Ordering::Relaxed);
            futures::future::ready(result)
        })
        .await
}
