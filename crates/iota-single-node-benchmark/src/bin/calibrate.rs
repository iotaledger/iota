// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Calibration entry point for the gas-metering work: the same validator,
//! account, and workload substrate as `iota-single-node-benchmark`, exposing
//! only the surface the calibration scripts drive. Two run modes, selected by
//! `--duration-secs`:
//!
//! - capture (default): transactions execute one at a time on the
//!   execution-only path and each measured transaction is written to
//!   `--profile-output`, so per-transaction timings are uncontended;
//! - sustained (`--duration-secs > 0`): rounds of the workload run through the
//!   real store commit path for write-side and headroom measurements.
//!
//! Signing is always skipped and no sample transactions are printed: neither
//! belongs in a timing measurement.

use std::path::PathBuf;

use clap::Parser;
use iota_single_node_benchmark::{
    command::{BenchmarkConfig, Component, WorkloadKind},
    init_telemetry, run_benchmark,
    workload::Workload,
};

#[derive(Parser)]
#[command(
    name = "calibrate",
    about = "Collect gas-metering calibration data on a single validator",
    author,
    version
)]
struct Calibrate {
    #[arg(
        long,
        default_value_t = 100,
        help = "Transactions to measure (per round in sustained mode)"
    )]
    tx_count: u64,
    #[arg(
        long,
        help = "One JSON line per measured transaction (digest, measured_ns, profile)"
    )]
    profile_output: Option<PathBuf>,
    #[arg(
        long,
        help = "Resident-memory readings for the measured phase, as JSON"
    )]
    rss_output: Option<PathBuf>,
    #[arg(
        long,
        default_value_t = 0,
        help = "Sustained mode: run rounds through the real store for this many seconds"
    )]
    duration_secs: u64,
    #[arg(long, help = "Persistent store directory for sustained mode")]
    db_path: Option<PathBuf>,
    #[arg(
        long,
        default_value_t = false,
        help = "Enable RocksDB write stalls (the write budget's signal)"
    )]
    enable_write_stall: bool,
    #[arg(long, help = "One JSON line per sustained-mode round")]
    stats_output: Option<PathBuf>,
    #[command(subcommand)]
    workload: WorkloadKind,
}

#[tokio::main]
async fn main() {
    let args = Calibrate::parse();

    let _telemetry_guard = init_telemetry(args.profile_output.as_ref());

    let sustained = args.duration_secs > 0;
    // Capture runs measure pure lane work on the in-memory execution path;
    // sustained runs need the real store because their subject is the commit
    // path itself.
    let component = if sustained {
        Component::Baseline
    } else {
        Component::ExecutionOnly
    };
    run_benchmark(
        Workload::new(args.tx_count, args.workload),
        component,
        BenchmarkConfig {
            checkpoint_size: 100,
            print_sample_tx: false,
            skip_signing: true,
            sequential: !sustained,
            duration_secs: args.duration_secs,
            db_path: args.db_path,
            enable_write_stall: args.enable_write_stall,
            stats_output: args.stats_output,
            rss_output: args.rss_output,
        },
    )
    .await;
}
