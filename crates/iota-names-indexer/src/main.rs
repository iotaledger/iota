// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod metrics;
mod worker;

use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use iota_names::config::IotaNamesConfig;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};
use tracing_subscriber::{
    EnvFilter, fmt::format::FmtSpan, layer::SubscriberExt, util::SubscriberInitExt,
};

use self::{
    metrics::{IotaNamesMetrics, PrometheusServer},
    worker::{IotaNamesWorker, run_iota_names_reader},
};

// Define the `GIT_REVISION` and `VERSION` consts
bin_version::bin_version!();

#[derive(Parser)]
#[command(
    name = env!("CARGO_BIN_NAME"),
    about = env!("CARGO_PKG_DESCRIPTION"),
    author,
    version = VERSION,
    propagate_version = true,
)]
enum Command {
    Start {
        /// The URL of an IOTA node to get data from.
        #[arg(long, default_value = "http://localhost:9000")]
        node_url: String,
        /// The number of workers to spawn in parallel.
        #[arg(long, default_value_t = 1)]
        num_workers: usize,
    },
}

impl Command {
    async fn execute(self) -> Result<()> {
        match self {
            Command::Start {
                node_url,
                num_workers,
            } => {
                info!("Starting IOTA Names Indexer");

                let prometheus = PrometheusServer::new();
                let registry = prometheus.registry();

                let cancel_token = CancellationToken::new();

                let mut tasks: JoinSet<Result<()>> = JoinSet::new();

                // Spawn the prometheus API
                let handle = cancel_token.clone();
                tasks.spawn(async move {
                    prometheus.start(handle).await?;
                    Ok(())
                });

                // Spawn the metrics worker
                let handle = cancel_token.clone();
                tasks.spawn(async move {
                    let worker = IotaNamesWorker::new(
                        IotaNamesConfig::from_env().unwrap_or_default(),
                        Arc::new(IotaNamesMetrics::new(&registry)),
                    );

                    tokio::select! {
                        res = run_iota_names_reader(worker, &node_url, &registry, handle.clone(), num_workers) => res?,
                        _ = handle.cancelled() => {},
                    }
                    Ok(())
                });

                let mut exit_code = Ok(());

                tokio::select! {
                    res = interrupt_or_terminate() => {
                        if let Err(err) = res {
                            tracing::error!("subscribing to OS interrupt signals failed with error: {err}; shutting down");
                            exit_code = Err(err);
                        } else {
                            tracing::info!("received interrupt; shutting down");
                        }
                    },
                    res = tasks.join_next() => {
                        if let Some(Ok(Err(err))) = res {
                            tracing::error!("a worker failed with error: {err}");
                            exit_code = Err(err);
                        }
                    },
                }

                cancel_token.cancel();

                // Allow the user to abort if the tasks aren't shutting down quickly.
                tokio::select! {
                    res = interrupt_or_terminate() => {
                        if let Err(err) = res {
                            tracing::error!("subscribing to OS interrupt signals failed with error: {err}; aborting");
                            exit_code = Err(err);
                        } else {
                            tracing::info!("received second interrupt; aborting");
                        }
                        tasks.shutdown().await;
                        tracing::info!("runtime aborted");
                    },
                    _ = async { while tasks.join_next().await.is_some() {} } => {
                        tracing::info!("runtime stopped");
                    },
                }

                exit_code
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    set_up_logging()?;

    Command::parse().execute().await?;
    Ok(())
}

fn set_up_logging() -> Result<()> {
    std::panic::set_hook(Box::new(|p| {
        error!("{}", p);
    }));

    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer().with_span_events(FmtSpan::CLOSE))
        .init();
    Ok(())
}

pub async fn interrupt_or_terminate() -> Result<()> {
    #[cfg(unix)]
    {
        use anyhow::anyhow;
        use tokio::signal::unix::{SignalKind, signal};
        let mut terminate = signal(SignalKind::terminate())
            .map_err(|e| anyhow!("cannot listen to `SIGTERM`: {e}"))?;
        let mut interrupt = signal(SignalKind::interrupt())
            .map_err(|e| anyhow!("cannot listen to `SIGINT`: {e}"))?;
        tokio::select! {
            _ = terminate.recv() => {}
            _ = interrupt.recv() => {}
        }
    }
    #[cfg(not(unix))]
    tokio::signal::ctrl_c().await?;

    Ok(())
}
