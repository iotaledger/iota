// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use clap::Parser;
use iota_single_node_benchmark::{
    command::{BenchmarkConfig, Command},
    init_telemetry, run_benchmark,
    workload::Workload,
};

#[tokio::main]
async fn main() {
    let args = Command::parse();

    let _telemetry_guard = init_telemetry(args.profile_output.as_ref());

    run_benchmark(
        Workload::new(args.tx_count, args.workload),
        args.component,
        BenchmarkConfig {
            checkpoint_size: args.checkpoint_size,
            print_sample_tx: args.print_sample_tx,
            skip_signing: args.skip_signing,
            sequential: args.sequential,
            duration_secs: args.duration_secs,
            db_path: args.db_path,
            enable_write_stall: args.enable_write_stall,
            stats_output: args.stats_output,
            rss_output: args.rss_output,
        },
    )
    .await;

    if std::env::var("TRACE_FILTER").is_ok() {
        println!("Sleeping for 60 seconds to allow tracing to flush.");
        println!("You can ctrl-c to exit once you see trace data appearing in grafana");
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
    }
}
