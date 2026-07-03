// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Per-runtime observability for the tokio runtimes the node runs on.
//!
//! The node splits work across separate tokio runtimes (notably a node-core
//! runtime and a client-facing serving runtime). These metrics make it possible
//! to tell whether one runtime is starving another for worker threads: the
//! `scheduler_lag_seconds` heartbeat and a non-zero `global_queue_depth` are
//! the direct signals of a runtime whose workers cannot keep up with ready
//! tasks.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use prometheus_filtered::{
    HistogramVec, IntGaugeVec, Registry, register_histogram_vec_with_registry,
    register_int_gauge_vec_with_registry,
};
use tokio::runtime::Handle;

/// How often the tokio runtime counters are sampled.
const SAMPLE_INTERVAL: Duration = Duration::from_secs(5);
/// How often the scheduler-lag heartbeat fires.
const HEARTBEAT_INTERVAL: Duration = Duration::from_millis(100);

pub struct RuntimeMonitorMetrics {
    workers: IntGaugeVec,
    alive_tasks: IntGaugeVec,
    global_queue_depth: IntGaugeVec,
    scheduler_lag_seconds: HistogramVec,
}

impl RuntimeMonitorMetrics {
    pub fn new(registry: &Registry) -> Arc<Self> {
        Arc::new(Self {
            workers: register_int_gauge_vec_with_registry!(
                "tokio_runtime_workers",
                "Number of worker threads in the tokio runtime.",
                &["runtime"],
                registry,
            )
            .unwrap(),
            alive_tasks: register_int_gauge_vec_with_registry!(
                "tokio_runtime_alive_tasks",
                "Number of alive (spawned, not yet completed) tasks in the tokio runtime.",
                &["runtime"],
                registry,
            )
            .unwrap(),
            global_queue_depth: register_int_gauge_vec_with_registry!(
                "tokio_runtime_global_queue_depth",
                "Tasks waiting in the runtime's global injection queue. A persistently \
                 non-zero value means the workers cannot keep up with ready tasks.",
                &["runtime"],
                registry,
            )
            .unwrap(),
            scheduler_lag_seconds: register_histogram_vec_with_registry!(
                "tokio_runtime_scheduler_lag_seconds",
                "Delay between when a fixed-interval heartbeat task was scheduled to wake \
                 and when it actually ran. High values indicate worker-thread starvation.",
                &["runtime"],
                vec![
                    0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0
                ],
                registry,
            )
            .unwrap(),
        })
    }
}

/// Starts background monitoring of the tokio runtime identified by `handle`,
/// labelling all metrics with `runtime`.
///
/// Both the counter sampler and the scheduler-lag heartbeat run *on the
/// monitored runtime* so that the heartbeat observes that runtime's scheduling
/// latency directly.
pub fn start_runtime_monitor(
    runtime: &'static str,
    handle: &Handle,
    metrics: Arc<RuntimeMonitorMetrics>,
) {
    // Sampler: periodically read the stable RuntimeMetrics counters.
    {
        let runtime_metrics = handle.metrics();
        let workers = metrics.workers.with_label_values(&[runtime]);
        let alive_tasks = metrics.alive_tasks.with_label_values(&[runtime]);
        let global_queue_depth = metrics.global_queue_depth.with_label_values(&[runtime]);
        handle.spawn(async move {
            loop {
                workers.set(runtime_metrics.num_workers() as i64);
                alive_tasks.set(runtime_metrics.num_alive_tasks() as i64);
                global_queue_depth.set(runtime_metrics.global_queue_depth() as i64);
                tokio::time::sleep(SAMPLE_INTERVAL).await;
            }
        });
    }

    // Heartbeat: the excess of the actual sleep duration over the intended
    // interval is the time the task spent waiting for a free worker thread.
    {
        let lag = metrics.scheduler_lag_seconds.with_label_values(&[runtime]);
        handle.spawn(async move {
            loop {
                let start = Instant::now();
                tokio::time::sleep(HEARTBEAT_INTERVAL).await;
                let lag_secs = start
                    .elapsed()
                    .saturating_sub(HEARTBEAT_INTERVAL)
                    .as_secs_f64();
                lag.observe(lag_secs);
            }
        });
    }
}
