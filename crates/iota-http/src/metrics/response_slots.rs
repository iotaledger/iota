// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The responses in progress on the connections of a listener.

use prometheus_filtered::{Histogram, MetricLevel, Registry, register_histogram_with_registry};

/// Responses on one connection. The buckets hold the values a limit is
/// likely set to.
const RESPONSES_PER_CONNECTION_BUCKETS: &[f64] =
    &[1.0, 2.0, 4.0, 8.0, 16.0, 32.0, 64.0, 100.0, 200.0];

pub(super) struct Metrics {
    responses_in_progress_per_connection: Histogram,
    responses_waiting_per_connection: Histogram,
}

impl Metrics {
    pub(super) fn new(prefix: &str, registry: &Registry, level: MetricLevel) -> Self {
        Self {
            responses_in_progress_per_connection: register_histogram_with_registry!(
                format!("{prefix}_responses_in_progress_per_connection"),
                "The responses in progress on a connection when one more starts, that one \
                 counted. A response is in progress from before its handler runs until its last \
                 byte has been written to the connection. Observed also with a limit of 0",
                RESPONSES_PER_CONNECTION_BUCKETS.to_vec(),
                registry;
                level
            )
            .expect("the metrics of a listener register without collision"),
            responses_waiting_per_connection: register_histogram_with_registry!(
                format!("{prefix}_responses_waiting_per_connection"),
                "The requests of a connection that wait for a response slot when one more \
                 starts to wait, that one counted",
                RESPONSES_PER_CONNECTION_BUCKETS.to_vec(),
                registry;
                level
            )
            .expect("the metrics of a listener register without collision"),
        }
    }

    pub(super) fn record_response_start(&self, in_progress: usize) {
        self.responses_in_progress_per_connection
            .observe(in_progress as f64);
    }

    pub(super) fn record_response_waiting(&self, waiting: usize) {
        self.responses_waiting_per_connection
            .observe(waiting as f64);
    }
}
