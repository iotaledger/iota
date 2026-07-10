// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Measures how long a fullnode takes to sync to a target checkpoint.
//!
//! Starts an `iota-node` binary, polls its gRPC API until the executed
//! checkpoint height reaches a target, reports the elapsed time, and shuts
//! the node down again. Intended for comparing sync performance of different
//! node builds against the same network.

use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::{Duration, Instant},
};

use anyhow::{Context, bail};
use clap::Parser;
use iota_config::{Config, NodeConfig};
use iota_grpc_client::{Client, ReadMask, read_mask_fields::ServiceInfoField};
use serde::Serialize;
use tokio::process::Child;

const SERVICE_INFO_FIELDS: &[&str] = &[
    ServiceInfoField::CHAIN,
    ServiceInfoField::EPOCH,
    ServiceInfoField::EXECUTED_CHECKPOINT_HEIGHT,
];
const STARTUP_POLL_INTERVAL: Duration = Duration::from_secs(2);
const NODE_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(60);

/// Measure how long a fullnode takes to sync to a target checkpoint.
///
/// Starts the given `iota-node` binary, polls the executed checkpoint height
/// via the gRPC `GetServiceInfo` call until it reaches the target, prints a
/// timing summary, and stops the node. The node config must have the gRPC
/// API enabled (`enable-grpc-api: true`). Note that the executed checkpoint
/// height trails the state-sync synced watermark slightly; runs are
/// comparable as long as they are all measured this way.
#[derive(Debug, Parser)]
pub struct MeasureSyncTime {
    /// Path to the iota-node binary to benchmark.
    #[arg(long)]
    node_binary: PathBuf,
    /// Fullnode config yaml, passed to the node via --config-path.
    #[arg(long)]
    config: PathBuf,
    /// Checkpoint sequence number at which the measurement ends.
    #[arg(long)]
    target_checkpoint: u64,
    /// gRPC API endpoint of the node under test.
    #[arg(long, default_value = "http://127.0.0.1:50051")]
    grpc_url: String,
    /// How often to poll sync progress.
    #[arg(long, default_value = "10s", value_parser = humantime::parse_duration)]
    poll_interval: Duration,
    /// Fail if the executed checkpoint height does not advance for this long.
    #[arg(long, value_parser = humantime::parse_duration)]
    stall_timeout: Option<Duration>,
    /// Delete the node's database directory (`db-path` from the config)
    /// before starting, so the run measures a sync from scratch.
    #[arg(long)]
    wipe_db: bool,
    /// Label identifying this run in the output (e.g. a branch name).
    #[arg(long)]
    label: Option<String>,
    /// Write a machine-readable JSON result to this path.
    #[arg(long)]
    json_output: Option<PathBuf>,
    /// File receiving the node's stdout and stderr. The node inherits the
    /// environment, so RUST_LOG controls its log level. Defaults to
    /// "iota-node-<label>.log" in the working directory.
    #[arg(long)]
    node_log_file: Option<PathBuf>,
    /// How long to wait after spawning the node for the gRPC API to serve
    /// its first response. Processing genesis on a fresh database can take
    /// several minutes.
    #[arg(long, default_value = "10m", value_parser = humantime::parse_duration)]
    startup_timeout: Duration,
}

/// Result of one measurement run, written as JSON when `--json-output` is
/// given.
#[derive(Debug, Serialize)]
struct SyncRunResult {
    label: Option<String>,
    node_binary: String,
    chain: Option<String>,
    target_checkpoint: u64,
    start_height: u64,
    end_height: u64,
    checkpoints_synced: u64,
    /// Node spawn until the first successful gRPC response.
    startup_secs: f64,
    /// First gRPC response until the target checkpoint was reached.
    sync_secs: f64,
    /// Node spawn until the target checkpoint was reached.
    total_secs: f64,
    avg_checkpoints_per_sec: f64,
    end_epoch: Option<u64>,
}

#[derive(Debug)]
struct NodeStatus {
    height: Option<u64>,
    epoch: Option<u64>,
    chain: Option<String>,
}

pub async fn run(args: MeasureSyncTime) -> anyhow::Result<()> {
    let node_config = NodeConfig::load(&args.config)
        .with_context(|| format!("failed to load node config from {}", args.config.display()))?;
    if !node_config.enable_grpc_api {
        bail!(
            "the node config does not enable the gRPC API; add this to {}:\n\
             enable-grpc-api: true\n\
             grpc-api-config:\n  address: \"127.0.0.1:50051\"",
            args.config.display()
        );
    }
    warn_on_port_mismatch(&args.grpc_url, &node_config);

    if args.wipe_db && node_config.db_path.exists() {
        println!(
            "Removing database directory {}",
            node_config.db_path.display()
        );
        std::fs::remove_dir_all(&node_config.db_path)?;
    }

    let log_path = args.node_log_file.clone().unwrap_or_else(|| {
        PathBuf::from(format!(
            "iota-node-{}.log",
            args.label.as_deref().unwrap_or("sync-run")
        ))
    });
    if let Some(parent) = log_path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    let log_file = std::fs::File::create(&log_path)
        .with_context(|| format!("failed to create node log file {}", log_path.display()))?;
    println!(
        "Starting {} (logs: {})",
        args.node_binary.display(),
        log_path.display()
    );

    let mut child = tokio::process::Command::new(&args.node_binary)
        .arg("--config-path")
        .arg(&args.config)
        .stdout(Stdio::from(log_file.try_clone()?))
        .stderr(Stdio::from(log_file))
        .kill_on_drop(true)
        .spawn()
        .with_context(|| format!("failed to start {}", args.node_binary.display()))?;
    let spawned_at = Instant::now();

    let client = Client::new(args.grpc_url.clone())?;
    let result = measure(&args, &client, &mut child, spawned_at, &log_path).await;
    // Stop the node also when the measurement failed, so no orphan process
    // keeps the database and ports busy.
    let shutdown_result = terminate_node(&mut child).await;
    let result = result?;
    shutdown_result?;

    println!("-------------------------------");
    if let Some(label) = &args.label {
        println!("Run label: {label}");
    }
    println!(
        "{}",
        format_summary(
            result.start_height,
            result.end_height,
            Duration::from_secs_f64(result.total_secs)
        )
    );

    if let Some(json_path) = &args.json_output {
        if let Some(parent) = json_path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        let file = std::fs::File::create(json_path)
            .with_context(|| format!("failed to create {}", json_path.display()))?;
        serde_json::to_writer_pretty(file, &result)?;
        println!("Result written to {}", json_path.display());
    }
    Ok(())
}

/// Waits for the node to serve gRPC, then polls until the target checkpoint
/// is reached. Does not stop the node; the caller does that in every case.
async fn measure(
    args: &MeasureSyncTime,
    client: &Client,
    child: &mut Child,
    spawned_at: Instant,
    log_path: &Path,
) -> anyhow::Result<SyncRunResult> {
    let shutdown = shutdown_signal();
    tokio::pin!(shutdown);

    // Startup phase: wait until the gRPC API reports an executed checkpoint
    // height for the first time.
    let first_status = loop {
        if let Some(status) = child.try_wait()? {
            bail!(
                "node exited during startup with {status}, see {}",
                log_path.display()
            );
        }
        if let Ok(status) = poll_status(client).await {
            if status.height.is_some() {
                break status;
            }
        }
        if spawned_at.elapsed() > args.startup_timeout {
            bail!(
                "node did not serve gRPC within {}, see {}",
                format_duration_secs(args.startup_timeout),
                log_path.display()
            );
        }
        tokio::select! {
            _ = tokio::time::sleep(STARTUP_POLL_INTERVAL) => {}
            _ = &mut shutdown => bail!("interrupted during node startup"),
        }
    };
    let first_response_at = Instant::now();
    let start_height = first_status.height.unwrap_or(0);
    println!(
        "Node is serving gRPC after {} at checkpoint {start_height} (target {})",
        format_duration_secs(spawned_at.elapsed()),
        args.target_checkpoint
    );
    if start_height >= args.target_checkpoint {
        bail!(
            "node is already at checkpoint {start_height}, at or beyond the target {}; \
             use --wipe-db or a higher --target-checkpoint",
            args.target_checkpoint
        );
    }

    let mut ticker = tokio::time::interval_at(
        (first_response_at + args.poll_interval).into(),
        args.poll_interval,
    );
    let mut last_height = start_height;
    let mut last_progress_at = Instant::now();
    let final_status = loop {
        tokio::select! {
            _ = ticker.tick() => {}
            status = child.wait() => {
                bail!("node exited with {status:?} before reaching the target checkpoint, see {}",
                    log_path.display());
            }
            _ = &mut shutdown => bail!("interrupted before reaching the target checkpoint"),
        }

        let status = match poll_status(client).await {
            Ok(status) => status,
            Err(e) => {
                eprintln!("sync progress poll failed (retrying): {e:#}");
                continue;
            }
        };
        let Some(height) = status.height else {
            continue;
        };

        if height > last_height {
            last_progress_at = Instant::now();
        } else if let Some(stall_timeout) = args.stall_timeout {
            if last_progress_at.elapsed() > stall_timeout {
                bail!(
                    "checkpoint height stuck at {height} for more than {}, see {}",
                    format_duration_secs(stall_timeout),
                    log_path.display()
                );
            }
        }

        let avg = rate(
            height.saturating_sub(start_height),
            first_response_at.elapsed(),
        );
        println!(
            "checkpoint {height}/{} | +{:.1} checkpoints/s (avg {avg:.1}) | epoch {} | ETA {}",
            args.target_checkpoint,
            rate(height.saturating_sub(last_height), args.poll_interval),
            status
                .epoch
                .map_or_else(|| "?".to_string(), |e| e.to_string()),
            eta(args.target_checkpoint.saturating_sub(height), avg)
                .map_or_else(|| "?".to_string(), format_duration_secs),
        );
        last_height = height;

        if height >= args.target_checkpoint {
            break status;
        }
    };

    let end_height = final_status.height.unwrap_or(last_height);
    let total = spawned_at.elapsed();
    let sync = first_response_at.elapsed();
    Ok(SyncRunResult {
        label: args.label.clone(),
        node_binary: args.node_binary.display().to_string(),
        chain: final_status.chain,
        target_checkpoint: args.target_checkpoint,
        start_height,
        end_height,
        checkpoints_synced: end_height.saturating_sub(start_height),
        startup_secs: (total - sync).as_secs_f64(),
        sync_secs: sync.as_secs_f64(),
        total_secs: total.as_secs_f64(),
        avg_checkpoints_per_sec: rate(end_height.saturating_sub(start_height), total),
        end_epoch: final_status.epoch,
    })
}

async fn poll_status(client: &Client) -> anyhow::Result<NodeStatus> {
    let envelope = client
        .get_service_info(Some(ReadMask::from(SERVICE_INFO_FIELDS)))
        .await?;
    let info = envelope.body();
    Ok(NodeStatus {
        height: info.executed_checkpoint_height,
        epoch: info.epoch,
        chain: info.chain.clone(),
    })
}

/// Resolves when the process receives ctrl-c or, on unix, SIGTERM (the
/// latter is what `docker compose stop` sends when this tool runs as the
/// container's main process).
async fn shutdown_signal() {
    #[cfg(unix)]
    {
        let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = sigterm.recv() => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

/// Stops the node, giving it time to shut down cleanly before killing it.
async fn terminate_node(child: &mut Child) -> anyhow::Result<()> {
    let Some(pid) = child.id() else {
        return Ok(());
    };
    println!("Stopping node (pid {pid})");
    // The node shuts down gracefully on SIGTERM; tokio's `kill` would send
    // SIGKILL and leave rocksdb to recover from the WAL on the next start.
    #[cfg(unix)]
    let sent = std::process::Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .status()
        .is_ok_and(|status| status.success());
    #[cfg(not(unix))]
    let sent = false;
    if !sent {
        child.start_kill()?;
    }
    match tokio::time::timeout(NODE_SHUTDOWN_TIMEOUT, child.wait()).await {
        Ok(status) => {
            status?;
        }
        Err(_) => {
            eprintln!(
                "node did not shut down within {}, killing it",
                format_duration_secs(NODE_SHUTDOWN_TIMEOUT)
            );
            child.kill().await?;
        }
    }
    Ok(())
}

fn warn_on_port_mismatch(grpc_url: &str, node_config: &NodeConfig) {
    let Some(config_port) = node_config
        .grpc_api_config
        .as_ref()
        .map(|config| config.address.port())
    else {
        return;
    };
    let url_port = grpc_url
        .trim_end_matches('/')
        .rsplit(':')
        .next()
        .and_then(|port| port.parse::<u16>().ok());
    if url_port.is_some_and(|port| port != config_port) {
        eprintln!(
            "warning: --grpc-url {grpc_url} does not match the gRPC port {config_port} in the \
             node config"
        );
    }
}

fn rate(checkpoints: u64, elapsed: Duration) -> f64 {
    if elapsed.is_zero() {
        return 0.0;
    }
    checkpoints as f64 / elapsed.as_secs_f64()
}

fn eta(remaining_checkpoints: u64, rate: f64) -> Option<Duration> {
    (rate > 0.0).then(|| Duration::from_secs_f64(remaining_checkpoints as f64 / rate))
}

/// Formats a duration truncated to whole seconds, e.g. "1h 35m 23s".
fn format_duration_secs(duration: Duration) -> String {
    humantime::format_duration(Duration::from_secs(duration.as_secs())).to_string()
}

fn format_summary(start_height: u64, end_height: u64, elapsed: Duration) -> String {
    let synced = end_height.saturating_sub(start_height);
    format!(
        "synced {synced} checkpoints ({start_height} -> {end_height}) in {}, avg {:.1} \
         checkpoints/s",
        format_duration_secs(elapsed),
        rate(synced, elapsed),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_is_zero_for_zero_elapsed_time() {
        assert_eq!(rate(1000, Duration::ZERO), 0.0);
    }

    #[test]
    fn rate_is_checkpoints_per_second() {
        assert_eq!(rate(1000, Duration::from_secs(10)), 100.0);
    }

    #[test]
    fn eta_is_none_at_zero_rate() {
        assert_eq!(eta(1000, 0.0), None);
    }

    #[test]
    fn eta_is_remaining_divided_by_rate() {
        assert_eq!(eta(1000, 100.0), Some(Duration::from_secs(10)));
    }

    #[test]
    fn durations_are_formatted_in_whole_seconds() {
        assert_eq!(
            format_duration_secs(Duration::from_millis(5_723_456)),
            "1h 35m 23s"
        );
    }

    #[test]
    fn summary_reports_height_range_duration_and_rate() {
        assert_eq!(
            format_summary(1_000, 601_000, Duration::from_secs(3600)),
            "synced 600000 checkpoints (1000 -> 601000) in 1h, avg 166.7 checkpoints/s"
        );
    }

    #[test]
    fn summary_handles_a_run_without_progress() {
        assert_eq!(
            format_summary(5, 5, Duration::from_secs(60)),
            "synced 0 checkpoints (5 -> 5) in 1m, avg 0.0 checkpoints/s"
        );
    }
}
