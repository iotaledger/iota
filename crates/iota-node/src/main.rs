// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{path::PathBuf, sync::Arc, time::Duration};

use clap::{ArgGroup, Parser};
use iota_common::sync::async_once_cell::AsyncOnceCell;
use iota_config::{Config, NodeConfig, node::RunWithRange};
use iota_core::runtime::IotaRuntimes;
use iota_metrics::hardware_metrics::{hardware_metrics_enabled, register_hardware_metrics};
use iota_node::{IotaNode, ServerVersion};
use iota_types::{
    committee::EpochId, crypto::KeypairTraits, messages_checkpoint::CheckpointSequenceNumber,
    multiaddr::Multiaddr, supported_protocol_versions::SupportedProtocolVersions,
};
#[cfg(all(feature = "flamegraph-alloc", nightly))]
use telemetry_subscribers::flamegraph::CounterAlloc;
use tokio::sync::broadcast;
use tracing::{error, info};

#[cfg(all(feature = "flamegraph-alloc", nightly))]
#[global_allocator]
static GLOBAL: CounterAlloc<std::alloc::System> = CounterAlloc::new(std::alloc::System);

// Define the `GIT_REVISION` and `VERSION` consts
bin_version::bin_version!();

#[derive(Parser)]
#[command(
    version = VERSION,
    group(ArgGroup::new("exclusive").required(false)), 
    name = env!("CARGO_BIN_NAME"))
]
struct Args {
    #[arg(long)]
    pub config_path: PathBuf,

    #[arg(long, help = "Specify address to listen on")]
    listen_address: Option<Multiaddr>,

    #[arg(long, group = "exclusive")]
    run_with_range_epoch: Option<EpochId>,

    #[arg(long, group = "exclusive")]
    run_with_range_checkpoint: Option<CheckpointSequenceNumber>,
}

fn main() {
    move_vm_profiler::ensure_move_vm_profiler_disabled();

    // Ensure that a validator never calls
    // get_for_min_version/get_for_max_version_UNSAFE. TODO: re-enable after we
    // figure out how to eliminate crashes in prod because of this.
    // ProtocolConfig::poison_get_for_min_version();

    let args = Args::parse();
    let mut config = NodeConfig::load(&args.config_path).unwrap();
    assert!(
        config.supported_protocol_versions.is_none(),
        "supported_protocol_versions cannot be read from the config file"
    );
    config.supported_protocol_versions = Some(SupportedProtocolVersions::SYSTEM_DEFAULT);

    // match run_with_range args
    // this means that we always modify the config used to start the node
    // for run_with_range. i.e if this is set in the config, it is ignored. only the
    // cli args enable/disable run_with_range
    match (args.run_with_range_epoch, args.run_with_range_checkpoint) {
        (None, Some(checkpoint)) => {
            config.run_with_range = Some(RunWithRange::Checkpoint(checkpoint))
        }
        (Some(epoch), None) => config.run_with_range = Some(RunWithRange::Epoch(epoch)),
        _ => config.run_with_range = None,
    };

    // Apply the configured metric group levels; an omitted `metrics.groups`
    // section behaves like the default config (dashboard metrics only).
    let metric_groups = config
        .metrics
        .as_ref()
        .and_then(|m| m.groups.clone())
        .unwrap_or_default();
    let metrics_filter =
        prometheus_filtered::Filter::resolve(Some(&metric_groups.to_filter_string()));

    let runtimes = IotaRuntimes::new(&config);
    let metrics_rt = runtimes.metrics.enter();
    let registry_service =
        iota_metrics::start_prometheus_server_with_filter(config.metrics_address, metrics_filter);

    // The hardware collector bypasses gather-time filtering, so its level is
    // applied here, once at startup: `off` skips the group, any other level
    // registers it.
    if hardware_metrics_enabled(&registry_service.default_registry().filter()) {
        register_hardware_metrics(&registry_service, &config.db_path)
            .expect("Failed registering hardware metrics");
    }

    // Initialize logging
    let prometheus_registry = registry_service.default_registry();
    let (_guard, tracing_handle) = telemetry_subscribers::TelemetryConfig::new()
        .with_env()
        .with_prom_registry(&prometheus_registry)
        .with_disable_span_latency(true)
        .init();

    drop(metrics_rt);

    info!("IOTA Node version: {VERSION}");
    info!(
        "Supported protocol versions: {:?}",
        config.supported_protocol_versions
    );

    info!(
        "Started Prometheus HTTP endpoint at {}",
        config.metrics_address
    );

    {
        let _enter = runtimes.metrics.enter();
        if let Some(metrics_config) = &config.metrics {
            if let Some(push_url) = &metrics_config.push_url {
                iota_metrics_push_client::start_metrics_push_task(
                    metrics_config.push_interval_seconds,
                    push_url.clone(),
                    config.network_key_pair().copy(),
                    registry_service.clone(),
                );
            }
        }
    }

    if let Some(listen_address) = args.listen_address {
        config.network_address = listen_address;
    }

    let admin_interface_address = config.admin_interface_address;

    // Run node in a separate runtime so that admin/monitoring functions continue to
    // work if it deadlocks.
    let node_once_cell = Arc::new(AsyncOnceCell::<Arc<IotaNode>>::new());
    let node_once_cell_clone = node_once_cell.clone();

    // let iota-node signal main to shutdown runtimes
    let (runtime_shutdown_tx, runtime_shutdown_rx) = broadcast::channel::<()>(1);

    // Client-facing servers run on a dedicated runtime so that external request
    // load never shares worker threads with the node core on `iota_node`.
    let serving_rt_handle = runtimes.serving.handle().clone();

    runtimes.iota_node.spawn(async move {
        let server_version = ServerVersion::new(env!("CARGO_BIN_NAME"), VERSION);
        match IotaNode::start_async(config, registry_service, server_version, serving_rt_handle)
            .await
        {
            Ok(iota_node) => node_once_cell_clone
                .set(iota_node)
                .expect("Failed to set node in AsyncOnceCell"),

            Err(e) => {
                error!("Failed to start node: {e:?}");
                std::process::exit(1);
            }
        }

        // get node, subscribe to shutdown channel
        let node = node_once_cell_clone.get().await;
        let mut shutdown_rx = node.subscribe_to_shutdown_channel();

        // when we get a shutdown signal from iota-node, forward it on to the
        // runtime_shutdown_channel here in main to signal all runtimes to shutdown.
        _ = shutdown_rx.recv().await;
        runtime_shutdown_tx
            .send(())
            .expect("failed to forward shutdown signal from iota-node to iota-node main");
        // TODO: Do we want to provide a way for the node to gracefully shutdown?
        loop {
            tokio::time::sleep(Duration::from_secs(1000)).await;
        }
    });

    runtimes.metrics.spawn(async move {
        let node = node_once_cell.get().await;

        iota_node::admin::run_admin_server(node, admin_interface_address, tracing_handle).await
    });

    // wait for SIGINT on the main thread
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(wait_termination(runtime_shutdown_rx));

    // Drop and wait all runtimes on main thread
    drop(runtimes);
}

#[cfg(not(unix))]
async fn wait_termination(mut shutdown_rx: broadcast::Receiver<()>) {
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {},
        _ = shutdown_rx.recv() => {},
    }
}

#[cfg(unix)]
async fn wait_termination(mut shutdown_rx: broadcast::Receiver<()>) {
    use futures::FutureExt;
    use tokio::signal::unix::*;

    let sigint = tokio::signal::ctrl_c().boxed();
    let mut sigterm = signal(SignalKind::terminate()).unwrap();
    let sigterm_recv = sigterm.recv().boxed();
    let shutdown_recv = shutdown_rx.recv().boxed();

    tokio::select! {
        _ = sigint => {},
        _ = sigterm_recv => {},
        _ = shutdown_recv => {},
    }
}
