// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use crate::{
    benchmark_context::BenchmarkContext,
    command::{BenchmarkConfig, Component},
    workload::Workload,
};

pub(crate) mod benchmark_context;
pub mod command;
pub(crate) mod mock_account;
pub(crate) mod mock_storage;
pub mod profile_capture;
pub mod rss;
pub(crate) mod single_node;
#[cfg(test)]
mod tests;
pub(crate) mod tx_generator;
pub mod workload;

/// Install the tracing setup the benchmark binaries share. With a profile
/// path, a subscriber that enables the `resource_profile` trace target and
/// routes its per-transaction events to the capture layer; otherwise the
/// standard telemetry setup, which keeps that target disabled. The returned
/// guards must stay alive for the duration of the run.
pub fn init_telemetry(
    profile_output: Option<&std::path::PathBuf>,
) -> Option<(
    telemetry_subscribers::TelemetryGuards,
    telemetry_subscribers::TracingHandle,
)> {
    let Some(path) = profile_output else {
        return Some(
            telemetry_subscribers::TelemetryConfig::new()
                .with_log_level("off,iota_single_node_benchmark=info")
                .with_env()
                .init(),
        );
    };
    use tracing_subscriber::{
        Layer, filter::Targets, layer::SubscriberExt, util::SubscriberInitExt,
    };

    let run_meta = serde_json::json!({
        "args": std::env::args().skip(1).collect::<Vec<_>>(),
        "version": env!("CARGO_PKG_VERSION"),
        "unix_time_secs": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock is after the unix epoch")
            .as_secs(),
    });
    let capture = profile_capture::ProfileCapture::new(path, run_meta)
        .unwrap_or_else(|e| panic!("failed to create {}: {e}", path.display()));
    tracing_subscriber::registry()
        .with(
            capture
                .with_filter(Targets::new().with_target("resource_profile", tracing::Level::TRACE)),
        )
        .with(tracing_subscriber::fmt::layer().with_filter(
            Targets::new().with_target("iota_single_node_benchmark", tracing::Level::INFO),
        ))
        .init();
    None
}

/// Benchmark a given workload on a specified component.
/// The different kinds of workloads and components can be found in command.rs.
/// \checkpoint_size represents both the size of a consensus commit, and size of
/// a checkpoint if we are benchmarking the checkpoint.
pub async fn run_benchmark(workload: Workload, component: Component, config: BenchmarkConfig) {
    // Only the measured workload below should reach --profile-output; the
    // setup transactions executed during context creation and certification
    // are suppressed.
    profile_capture::set_capture_enabled(false);
    let mut ctx = BenchmarkContext::new(workload.clone(), component, &config).await;
    let tx_generator = workload.create_tx_generator(&mut ctx).await;
    if config.duration_secs > 0 {
        ctx.benchmark_sustained_execution(tx_generator, &config)
            .await;
        return;
    }
    let transactions = ctx.generate_transactions(tx_generator).await;
    if matches!(component, Component::TxnSigning) {
        ctx.benchmark_transaction_signing(transactions, config.print_sample_tx)
            .await;
        return;
    }

    let transactions = ctx
        .certify_transactions(transactions, config.skip_signing)
        .await;
    ctx.validator()
        .assigned_shared_object_versions(&transactions)
        .await;
    profile_capture::set_capture_enabled(true);
    // Baseline right before the measured phase: the lifetime peak minus this
    // is the phase's memory footprint (the response variable for the memory
    // scale factors).
    let rss_baseline = rss::current_rss_bytes();
    match component {
        Component::CheckpointExecutor => {
            ctx.benchmark_checkpoint_executor(transactions, config.checkpoint_size)
                .await;
        }
        Component::ExecutionOnly => {
            ctx.benchmark_transaction_execution_in_memory(transactions, config.print_sample_tx)
                .await;
        }
        _ => {
            ctx.benchmark_transaction_execution(transactions, config.print_sample_tx)
                .await;
        }
    }
    if let Some(path) = &config.rss_output {
        let peak = rss::peak_rss_bytes();
        let readings = serde_json::json!({
            "baseline_bytes": rss_baseline,
            "peak_bytes": peak,
            "delta_bytes": peak.saturating_sub(rss_baseline),
            "peak_before_phase": peak <= rss_baseline,
        });
        std::fs::write(path, format!("{readings}\n")).expect("failed to write --rss-output");
    }
}
