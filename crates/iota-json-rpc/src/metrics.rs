// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashSet;

use iota_json_rpc_api::TRANSIENT_ERROR_CODE;
use jsonrpsee::{MethodKind, types::Params};
use prometheus_filtered::{
    HistogramVec, IntCounterVec, IntGaugeVec, MetricLevel, register_histogram_vec_with_registry,
    register_int_counter_vec_with_registry, register_int_gauge_vec_with_registry,
};
use tokio::time::Instant;

use crate::logger::Logger;

const SPAM_LABEL: &str = "SPAM";
const LATENCY_SEC_BUCKETS: &[f64] = &[
    0.001, 0.005, 0.01, 0.05, 0.1, 0.25, 0.5, 1., 2.5, 5., 10., 20., 30., 60., 90.,
];

#[derive(Debug, Clone)]
pub struct Metrics {
    /// Counter of requests, route is a label (ie separate timeseries per route)
    requests_by_route: IntCounterVec,
    /// Gauge of inflight requests, route is a label (ie separate timeseries per
    /// route)
    inflight_requests_by_route: IntGaugeVec,
    /// Request latency, route is a label
    req_latency_by_route: HistogramVec,
    /// Failed requests by route
    errors_by_route: IntCounterVec,
    server_errors_by_route: IntCounterVec,
    client_errors_by_route: IntCounterVec,
    transient_errors_by_route: IntCounterVec,
}

#[derive(Clone)]
pub struct MetricsLogger {
    metrics: Metrics,
    method_whitelist: HashSet<String>,
}

impl MetricsLogger {
    fn check_spam<'a>(&'a self, method_name: &'a str) -> &'a str {
        if self.method_whitelist.contains(method_name) {
            method_name
        } else {
            SPAM_LABEL
        }
    }

    pub fn new(registry: &prometheus_filtered::Registry, method_whitelist: &[&str]) -> Self {
        let metrics = Metrics {
            requests_by_route: register_int_counter_vec_with_registry!(
                "rpc_requests_by_route",
                "Number of requests by route",
                &["route"],
                registry;
                MetricLevel::Warn,
            )
            .unwrap(),
            inflight_requests_by_route: register_int_gauge_vec_with_registry!(
                "inflight_rpc_requests_by_route",
                "Number of inflight requests by route",
                &["route"],
                registry;
                MetricLevel::Warn,
            )
            .unwrap(),
            req_latency_by_route: register_histogram_vec_with_registry!(
                "req_latency_by_route",
                "Latency of a request by route",
                &["route"],
                LATENCY_SEC_BUCKETS.to_vec(),
                registry;
                MetricLevel::Warn,
            )
            .unwrap(),
            client_errors_by_route: register_int_counter_vec_with_registry!(
                "client_errors_by_route",
                "Number of client errors by route",
                &["route"],
                registry,
            )
            .unwrap(),
            server_errors_by_route: register_int_counter_vec_with_registry!(
                "server_errors_by_route",
                "Number of server errors by route",
                &["route"],
                registry,
            )
            .unwrap(),
            transient_errors_by_route: register_int_counter_vec_with_registry!(
                "transient_errors_by_route",
                "Number of transient errors by route",
                &["route"],
                registry,
            )
            .unwrap(),
            errors_by_route: register_int_counter_vec_with_registry!(
                "errors_by_route",
                "Number of client and server errors by route",
                &["route"],
                registry;
                MetricLevel::Warn,
            )
            .unwrap(),
        };

        Self {
            metrics,
            method_whitelist: method_whitelist.iter().map(|s| (*s).into()).collect(),
        }
    }
}

impl Logger for MetricsLogger {
    type Instant = Instant;

    fn on_request(&self) -> Self::Instant {
        Instant::now()
    }

    fn on_call(&self, method_name: &str, _params: Params, _kind: MethodKind) {
        let method_name = self.check_spam(method_name);
        self.metrics
            .inflight_requests_by_route
            .with_label_values(&[method_name])
            .inc();
        self.metrics
            .requests_by_route
            .with_label_values(&[method_name])
            .inc();
    }

    fn on_result(
        &self,
        method_name: &str,
        _success: bool,
        error_code: Option<i32>,
        started_at: Self::Instant,
    ) {
        let method_name = self.check_spam(method_name);
        self.metrics
            .inflight_requests_by_route
            .with_label_values(&[method_name])
            .dec();
        let req_latency_secs = (Instant::now() - started_at).as_secs_f64();
        self.metrics
            .req_latency_by_route
            .with_label_values(&[method_name])
            .observe(req_latency_secs);

        if let Some(code) = error_code {
            if code == jsonrpsee::types::error::CALL_EXECUTION_FAILED_CODE
                || code == jsonrpsee::types::error::INTERNAL_ERROR_CODE
            {
                self.metrics
                    .server_errors_by_route
                    .with_label_values(&[method_name])
                    .inc();
            } else if code == TRANSIENT_ERROR_CODE {
                self.metrics
                    .transient_errors_by_route
                    .with_label_values(&[method_name])
                    .inc();
            } else {
                self.metrics
                    .client_errors_by_route
                    .with_label_values(&[method_name])
                    .inc();
            }
            self.metrics
                .errors_by_route
                .with_label_values(&[method_name])
                .inc();
        }
    }
}
