// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Backfills a checkpoint range from a historical store and logs what it commits.
//!
//! ```sh
//! cargo run --bin backfill -- 1000 1100 desc
//! ```
//!
//! The arguments are the first and the last checkpoint of the range, the order and the URL of
//! the historical store. The order is `asc` or `desc` and defaults to `asc`. The URL defaults to
//! the testnet store. Ctrl+C stops the run gracefully. Set `RUST_LOG=info,backfill=debug` to also
//! see every processed checkpoint.

use std::{num::NonZeroUsize, sync::Arc};

use anyhow::{Context, bail};
use async_trait::async_trait;
use iota_data_ingestion_core::{
    BackfillExecutor, BackfillOptions, IngestionError, IngestionOrder, Reducer, Worker,
    reader::v2::RemoteUrl,
};
use iota_types::{
    full_checkpoint_content::CheckpointData, messages_checkpoint::CheckpointSequenceNumber,
};
use tokio_util::sync::CancellationToken;
use tracing::{debug, info};
use tracing_subscriber::EnvFilter;

const TESTNET_HISTORICAL_STORE: &str = "https://checkpoints.testnet.iota.cafe/ingestion/historical";
const USAGE: &str = "usage: backfill <first> <last> [asc|desc] [historical store url]";
const WORKER_CONCURRENCY: NonZeroUsize = NonZeroUsize::new(4).unwrap();

/// Returns the number of each checkpoint and how many transactions it holds.
struct LoggingWorker;

#[async_trait]
impl Worker for LoggingWorker {
    type Message = (CheckpointSequenceNumber, usize);
    type Error = IngestionError;

    async fn process_checkpoint(
        &self,
        checkpoint: Arc<CheckpointData>,
    ) -> Result<Self::Message, Self::Error> {
        let sequence_number = checkpoint.checkpoint_summary.sequence_number;
        debug!("processed checkpoint {sequence_number}");
        Ok((sequence_number, checkpoint.transactions.len()))
    }
}

/// Logs every batch it is asked to commit, so the order of the commits is visible.
struct LoggingReducer;

#[async_trait]
impl Reducer<LoggingWorker> for LoggingReducer {
    async fn commit(
        &self,
        batch: &[(CheckpointSequenceNumber, usize)],
    ) -> Result<(), IngestionError> {
        let checkpoints = batch
            .iter()
            .map(|(checkpoint, _)| checkpoint)
            .collect::<Vec<_>>();
        let transactions: usize = batch.iter().map(|(_, transactions)| transactions).sum();
        info!("committed checkpoints {checkpoints:?} with {transactions} transactions");
        Ok(())
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let mut args = std::env::args().skip(1);
    let first: CheckpointSequenceNumber = args.next().context(USAGE)?.parse().context(USAGE)?;
    let last: CheckpointSequenceNumber = args.next().context(USAGE)?.parse().context(USAGE)?;
    if first > last {
        bail!("<first> must not be larger than <last>, also for `desc`. {USAGE}");
    }
    let order = match args.next().as_deref() {
        None | Some("asc") => IngestionOrder::Ascending,
        Some("desc") => IngestionOrder::Descending,
        Some(other) => bail!("unknown order `{other}`, {USAGE}"),
    };
    let historical_url = args
        .next()
        .unwrap_or_else(|| TESTNET_HISTORICAL_STORE.into());

    // Ctrl+C cancels the token, and the executor then shuts down gracefully.
    let token = CancellationToken::new();
    tokio::spawn({
        let token = token.clone();
        async move {
            if tokio::signal::ctrl_c().await.is_ok() {
                info!("ctrl-c received, stopping the backfill");
                token.cancel();
            }
        }
    });

    let options = BackfillOptions::default()
        .with_order(order)
        .with_worker_concurrency(WORKER_CONCURRENCY);
    let remote_url = RemoteUrl::HybridHistoricalStore {
        historical_url,
        live_url: None,
    };
    info!("backfilling checkpoints {first}..={last} in {order:?} order");
    let executor = BackfillExecutor::new_with_reducer(
        remote_url,
        LoggingWorker,
        options,
        token,
        LoggingReducer,
    )?;

    let last_processed = executor.run(first..=last).await?;
    info!("backfill ended, last processed checkpoint is {last_processed:?}");
    Ok(())
}
