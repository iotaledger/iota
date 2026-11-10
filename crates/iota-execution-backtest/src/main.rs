// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Re-executes the user programmable transactions of a range of checkpoints with two execution
//! versions (a control and a candidate) and compares the effects of each with the effects the
//! chain recorded.
//!
//! Checkpoints are streamed from the historical checkpoint archive and kept in memory only while
//! they are processed. Objects a checkpoint does not carry are read from a JSON-RPC full node at
//! the versions the transaction read, and dropped with the transaction.

mod replay;
mod rpc;
mod store;

use std::{
    collections::BTreeMap,
    fs::File,
    io::{BufWriter, Write},
    num::NonZeroUsize,
    path::PathBuf,
    sync::Arc,
};

use anyhow::{Context, Result, bail};
use clap::Parser;
use futures::StreamExt;
use iota_config::object_storage_config::{ObjectStoreConfig, ObjectStoreType};
use iota_data_ingestion_core::history::{
    epoch_boundaries::EpochBoundaries,
    reader::{HistoricalReader, HistoricalReaderConfig, make_blob_iterator_for_range},
};
use iota_protocol_config::{Chain, ProtocolConfig, ProtocolVersion};
use iota_sdk_types::{EndOfEpochTransactionKind, ObjectId, TransactionKind, Version};
use iota_types::{
    full_checkpoint_content::CheckpointData, metrics::LimitsMetrics, object::Object,
    transaction::TransactionAPI,
};
use tokio::{sync::Semaphore, task::JoinSet};
use tracing::{info, warn};

use crate::{
    replay::{EpochContext, TransactionReport, is_user_programmable, replay_transaction},
    rpc::Rpc,
    store::PackageCache,
};

/// Most parallel archive downloads allowed.
const MAX_DOWNLOAD_CONCURRENCY: usize = 10;
/// Consecutive failed archive downloads before the run stops.
const MAX_DOWNLOAD_ATTEMPTS: u32 = 5;
/// Delay before retrying a failed archive download, multiplied by the attempt number.
const DOWNLOAD_RETRY_DELAY: std::time::Duration = std::time::Duration::from_secs(10);
/// Most JSON-RPC requests per second allowed.
const MAX_RPC_REQUESTS_PER_SECOND: u32 = 100;

#[derive(Parser)]
#[command(about = "Re-execute user transactions with two execution versions and compare effects")]
struct Args {
    /// Epoch whose checkpoints are replayed.
    #[arg(long, default_value_t = 10)]
    epoch: u64,
    /// Number of checkpoints to replay, from the first checkpoint of the epoch (or `--start`).
    #[arg(long, default_value_t = 10_000)]
    checkpoints: u64,
    /// First checkpoint to replay; must lie in the epoch. Defaults to the epoch's first.
    #[arg(long)]
    start: Option<u64>,
    /// Stop after this many user transactions.
    #[arg(long)]
    max_transactions: Option<usize>,
    /// Historical checkpoint archive (S3 compatible, unsigned requests).
    #[arg(
        long,
        default_value = "https://checkpoints.devnet.iota.cafe/ingestion/historical"
    )]
    archive_url: String,
    #[arg(long, default_value = "weur")]
    archive_region: String,
    /// JSON-RPC full node used for objects the checkpoints do not carry.
    #[arg(long, default_value = "https://api.devnet.iota.cafe")]
    rpc_url: String,
    /// Parallel archive downloads (at most 10).
    #[arg(long, default_value_t = 4)]
    download_concurrency: usize,
    /// JSON-RPC requests per second (at most 100).
    #[arg(long, default_value_t = 90)]
    rpc_requests_per_second: u32,
    /// Transactions replayed in parallel.
    #[arg(long, default_value_t = 8)]
    transaction_concurrency: usize,
    /// Execution version of the candidate (default: the `vm2` cut).
    #[arg(long, default_value_t = iota_execution::VM2)]
    candidate_execution_version: u64,
    /// Report file: one JSON line per transaction.
    #[arg(long)]
    out: PathBuf,
    /// Append to the report file instead of replacing it (to continue an interrupted run).
    #[arg(long)]
    append: bool,
    /// Only measure the archive data of the range (bytes, checkpoints, transactions) and print the
    /// checkpoint count of every epoch; nothing is replayed.
    #[arg(long)]
    sizes_only: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let _guard = telemetry_subscribers::TelemetryConfig::new()
        .with_env()
        .init();
    let args = Args::parse();
    if args.download_concurrency == 0 || args.download_concurrency > MAX_DOWNLOAD_CONCURRENCY {
        bail!("--download-concurrency must be between 1 and {MAX_DOWNLOAD_CONCURRENCY}");
    }
    if args.rpc_requests_per_second == 0
        || args.rpc_requests_per_second > MAX_RPC_REQUESTS_PER_SECOND
    {
        bail!("--rpc-requests-per-second must be between 1 and {MAX_RPC_REQUESTS_PER_SECOND}");
    }

    let reader = HistoricalReader::new(HistoricalReaderConfig {
        remote_store_config: ObjectStoreConfig {
            object_store: Some(ObjectStoreType::S3),
            object_store_connection_limit: args.download_concurrency,
            aws_endpoint: Some(args.archive_url.clone()),
            aws_virtual_hosted_style_request: true,
            aws_region: Some(args.archive_region.clone()),
            no_sign_request: true,
            ..Default::default()
        },
        download_concurrency: NonZeroUsize::new(args.download_concurrency)
            .context("download concurrency")?,
    })?;
    reader.sync_manifest_once().await?;
    let boundaries = reader.epoch_boundaries().await?;
    let previous_epoch_end = if args.epoch == 0 {
        bail!("epoch 0 has no previous epoch to read its configuration from");
    } else {
        boundaries
            .get(args.epoch - 1)
            .with_context(|| format!("end of epoch {} not in the archive", args.epoch - 1))?
    };
    let epoch_first = previous_epoch_end + 1;
    let epoch_last = boundaries
        .get(args.epoch)
        .unwrap_or(reader.latest_available_checkpoint().await?);
    let start = args.start.unwrap_or(epoch_first);
    if start < epoch_first || start > epoch_last {
        bail!(
            "--start {start} is outside epoch {} ({epoch_first}..={epoch_last})",
            args.epoch
        );
    }
    let end = (start + args.checkpoints).min(epoch_last + 1);
    if args.sizes_only {
        return print_sizes(&reader, &boundaries, start..end).await;
    }
    info!(epoch = args.epoch, start, end, "replaying checkpoints");

    // The last checkpoint of the previous epoch holds the change to this epoch.
    let change_checkpoint = reader
        .iter_for_range(previous_epoch_end..previous_epoch_end + 1)
        .await?
        .next()
        .context("end-of-epoch checkpoint not found")?;
    let (protocol_version, epoch_start_timestamp_ms) = epoch_change(&change_checkpoint)?;
    drop(change_checkpoint);
    let protocol_config =
        ProtocolConfig::get_for_version(ProtocolVersion::new(protocol_version), Chain::Unknown);
    // From protocol version 5 the reference gas price is the protocol's base gas price.
    let reference_gas_price = protocol_config.base_gas_price();
    let mut candidate_config = protocol_config.clone();
    candidate_config.set_execution_version_for_testing(args.candidate_execution_version);
    info!(
        protocol_version,
        epoch_start_timestamp_ms, reference_gas_price, "epoch configuration"
    );

    let rpc = Arc::new(Rpc::new(&args.rpc_url, args.rpc_requests_per_second).await?);
    let system_packages = system_packages_at(&rpc, start).await?;
    let registry = prometheus_filtered::Registry::new();
    let ctx = Arc::new(EpochContext {
        epoch: args.epoch,
        epoch_start_timestamp_ms,
        reference_gas_price,
        control: iota_execution::executor(&protocol_config, true, None)?,
        candidate: iota_execution::executor(&candidate_config, true, None)?,
        protocol_config,
        packages: Arc::new(PackageCache::new(rpc.clone(), system_packages)),
        rpc,
        metrics: Arc::new(LimitsMetrics::new(&registry)),
    });

    let out_file = if args.append {
        File::options().create(true).append(true).open(&args.out)
    } else {
        File::create(&args.out)
    };
    let mut out =
        BufWriter::new(out_file.with_context(|| format!("opening {}", args.out.display()))?);
    let mut summary = Summary::default();
    let semaphore = Arc::new(Semaphore::new(args.transaction_concurrency.max(1)));
    // A failed download restarts the stream after the last replayed checkpoint.
    let mut next_checkpoint = start;
    let mut failed_attempts = 0;
    'stream: while next_checkpoint < end {
        let mut blobs = reader.stream_blobs_for_range(next_checkpoint..end).await?;
        while let Some(blob) = blobs.next().await {
            let blob = match blob {
                Ok(blob) => blob,
                Err(e) => {
                    failed_attempts += 1;
                    if failed_attempts > MAX_DOWNLOAD_ATTEMPTS {
                        return Err(e).context("downloading checkpoints");
                    }
                    warn!(
                        next_checkpoint,
                        failed_attempts, "checkpoint download failed, retrying: {e}"
                    );
                    tokio::time::sleep(DOWNLOAD_RETRY_DELAY * failed_attempts).await;
                    continue 'stream;
                }
            };
            failed_attempts = 0;
            // The blob holds a batch of checkpoints; it is dropped after its checkpoints are
            // replayed.
            for checkpoint in make_blob_iterator_for_range(blob, next_checkpoint..end)? {
                let sequence_number = checkpoint.checkpoint_summary.sequence_number;
                next_checkpoint = sequence_number + 1;
                summary.checkpoints += 1;
                let mut tasks = JoinSet::new();
                for tx in checkpoint
                    .transactions
                    .into_iter()
                    .filter(is_user_programmable)
                {
                    if args
                        .max_transactions
                        .is_some_and(|max| summary.transactions >= max)
                    {
                        break;
                    }
                    summary.transactions += 1;
                    let permit = semaphore.clone().acquire_owned().await?;
                    let ctx = ctx.clone();
                    tasks.spawn(async move {
                        let report = replay_transaction(ctx, sequence_number, tx).await;
                        drop(permit);
                        report
                    });
                }
                let mut reports = tasks.join_all().await;
                reports.sort_by(|a, b| a.digest.cmp(&b.digest));
                for report in reports {
                    summary.add(&report);
                    serde_json::to_writer(&mut out, &report)?;
                    out.write_all(b"\n")?;
                }
                out.flush()?;
                if sequence_number % 100 == 0 {
                    info!(checkpoint = sequence_number, ?summary, "progress");
                }
                if args
                    .max_transactions
                    .is_some_and(|max| summary.transactions >= max)
                {
                    break 'stream;
                }
            }
        }
    }
    println!("{}", serde_json::to_string_pretty(&summary)?);
    Ok(())
}

/// Prints the checkpoint count of every epoch in the archive, and the archive bytes, checkpoints
/// and transactions of `range`.
async fn print_sizes(
    reader: &HistoricalReader,
    boundaries: &EpochBoundaries,
    range: std::ops::Range<u64>,
) -> Result<()> {
    let mut previous_end: Option<u64> = None;
    for epoch in 0.. {
        let Some(last) = boundaries.get(epoch) else {
            break;
        };
        let first = previous_end.map_or(0, |end| end + 1);
        println!(
            "epoch {epoch}: checkpoints {first}..={last} ({})",
            last + 1 - first
        );
        previous_end = Some(last);
    }
    let (mut bytes, mut checkpoints, mut transactions, mut user_transactions) = (0, 0, 0, 0);
    let mut decoded_bytes = 0;
    let mut blobs = reader.stream_blobs_for_range(range.clone()).await?;
    while let Some(blob) = blobs.next().await {
        let blob = blob?;
        bytes += blob.len();
        for checkpoint in make_blob_iterator_for_range(blob, range.clone())? {
            checkpoints += 1;
            decoded_bytes += bcs::serialized_size(&checkpoint)?;
            transactions += checkpoint.transactions.len();
            user_transactions += checkpoint
                .transactions
                .iter()
                .filter(|tx| is_user_programmable(tx))
                .count();
        }
    }
    println!(
        "range {range:?}: {bytes} archive bytes (whole batch files), {decoded_bytes} decoded \
         bytes, {checkpoints} checkpoints, {transactions} transactions, {user_transactions} user \
         transactions"
    );
    Ok(())
}

/// The protocol version and start timestamp of the epoch that the end-of-epoch transaction in
/// `checkpoint` starts.
fn epoch_change(checkpoint: &CheckpointData) -> Result<(u64, u64)> {
    for tx in &checkpoint.transactions {
        let TransactionKind::EndOfEpoch(kinds) = tx.transaction.data().transaction().kind() else {
            continue;
        };
        for kind in kinds {
            match kind {
                EndOfEpochTransactionKind::ChangeEpoch(change) => {
                    return Ok((change.protocol_version, change.epoch_start_timestamp_ms));
                }
                EndOfEpochTransactionKind::ChangeEpochV2(change) => {
                    return Ok((change.protocol_version, change.epoch_start_timestamp_ms));
                }
                EndOfEpochTransactionKind::ChangeEpochV3(change) => {
                    return Ok((change.protocol_version, change.epoch_start_timestamp_ms));
                }
                EndOfEpochTransactionKind::ChangeEpochV4(change) => {
                    return Ok((change.protocol_version, change.epoch_start_timestamp_ms));
                }
                _ => {}
            }
        }
    }
    bail!(
        "checkpoint {} has no epoch change",
        checkpoint.checkpoint_summary.sequence_number
    )
}

/// The system packages as they were at checkpoint `checkpoint`. System packages are upgraded in
/// place, so each version is checked against the checkpoint of the transaction that wrote it.
async fn system_packages_at(rpc: &Rpc, checkpoint: u64) -> Result<BTreeMap<ObjectId, Object>> {
    let mut packages = BTreeMap::new();
    for id in [
        ObjectId::STD,
        ObjectId::FRAMEWORK,
        ObjectId::SYSTEM,
        ObjectId::STARDUST,
    ] {
        let mut package = rpc
            .latest_object(id)
            .await?
            .with_context(|| format!("system package {id} not found"))?;
        loop {
            let written_at = rpc
                .transaction_checkpoint(package.previous_transaction)
                .await?;
            // Genesis objects are written by a transaction without a checkpoint lookup result.
            if written_at.is_none_or(|written_at| written_at < checkpoint) {
                break;
            }
            let previous_version =
                Version::from_u64(package.version().as_u64().checked_sub(1).with_context(
                    || format!("no version of {id} before checkpoint {checkpoint}"),
                )?);
            package = rpc
                .object_at_version(id, previous_version)
                .await?
                .with_context(|| format!("system package {id} version {previous_version}"))?;
        }
        info!(%id, version = package.version().as_u64(), "system package");
        packages.insert(id, package);
    }
    Ok(packages)
}

#[derive(Default, Debug, serde::Serialize)]
struct Summary {
    checkpoints: u64,
    transactions: usize,
    skipped: usize,
    control_matches: usize,
    candidate_matches: usize,
    /// Control matches the chain but the candidate does not: a divergence of the candidate.
    candidate_diverged: usize,
    /// Control does not match the chain: the replay itself is off for this transaction.
    control_diverged: usize,
    with_move_authenticators: usize,
}

impl Summary {
    fn add(&mut self, report: &TransactionReport) {
        if report.move_authenticators > 0 {
            self.with_move_authenticators += 1;
        }
        let (Some(control), Some(candidate)) = (&report.control, &report.candidate) else {
            self.skipped += 1;
            return;
        };
        if control.matches {
            self.control_matches += 1;
        } else {
            self.control_diverged += 1;
        }
        if candidate.matches {
            self.candidate_matches += 1;
        }
        if control.matches && !candidate.matches {
            self.candidate_diverged += 1;
        }
    }
}
