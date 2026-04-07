// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashSet, time::Duration};

use iota_common::metrics::{MetricsPushClient, push_metrics};
use iota_grpc_server::metrics::{LATENCY_SEC_BUCKETS, SPAM_LABEL, grpc_code_to_str};
use iota_metrics::RegistryService;
use iota_network::{api::VALIDATOR_METHOD_PATHS, tonic::Code};
use iota_network_stack::metrics::MetricsCallbackProvider;
use prometheus::{
    HistogramVec, IntCounterVec, IntGaugeVec, Registry, register_histogram_vec_with_registry,
    register_int_counter_vec_with_registry, register_int_gauge_vec_with_registry,
};

/// Starts a task to periodically push metrics to a configured endpoint if a
/// metrics push endpoint is configured.
pub fn start_metrics_push_task(config: &iota_config::NodeConfig, registry: RegistryService) {
    use fastcrypto::traits::KeyPair;
    use iota_config::node::MetricsConfig;

    const DEFAULT_METRICS_PUSH_INTERVAL: Duration = Duration::from_secs(60);

    let (interval, url) = match &config.metrics {
        Some(MetricsConfig {
            push_interval_seconds,
            push_url: Some(url),
        }) => {
            let interval = push_interval_seconds
                .map(Duration::from_secs)
                .unwrap_or(DEFAULT_METRICS_PUSH_INTERVAL);
            let url = reqwest::Url::parse(url).expect("unable to parse metrics push url");
            (interval, url)
        }
        _ => return,
    };

    // make a copy so we can make a new client later when we hit errors posting
    // metrics
    let config_copy = config.clone();
    let mut client = MetricsPushClient::new(config_copy.network_key_pair().copy());

    tokio::spawn(async move {
        tracing::info!(push_url =% url, interval =? interval, "Started Metrics Push Service");

        let mut interval = tokio::time::interval(interval);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        let mut errors = 0;
        loop {
            interval.tick().await;

            if let Err(error) = push_metrics(&client, &url, &registry).await {
                errors += 1;
                if errors >= 10 {
                    // If we hit 10 failures in a row, start logging errors.
                    tracing::error!("unable to push metrics: {error}; new client will be created");
                } else {
                    tracing::warn!("unable to push metrics: {error}; new client will be created");
                }
                // aggressively recreate our client connection if we hit an error
                client = MetricsPushClient::new(config_copy.network_key_pair().copy());
            } else {
                errors = 0;
            }
        }
    });
}

pub struct IotaNodeMetrics {
    pub jwk_requests: IntCounterVec,
    pub jwk_request_errors: IntCounterVec,

    pub total_jwks: IntCounterVec,
    pub invalid_jwks: IntCounterVec,
    pub unique_jwks: IntCounterVec,
}

impl IotaNodeMetrics {
    pub fn new(registry: &Registry) -> Self {
        Self {
            jwk_requests: register_int_counter_vec_with_registry!(
                "jwk_requests",
                "Total number of JWK requests",
                &["provider"],
                registry,
            )
            .unwrap(),
            jwk_request_errors: register_int_counter_vec_with_registry!(
                "jwk_request_errors",
                "Total number of JWK request errors",
                &["provider"],
                registry,
            )
            .unwrap(),
            total_jwks: register_int_counter_vec_with_registry!(
                "total_jwks",
                "Total number of JWKs",
                &["provider"],
                registry,
            )
            .unwrap(),
            invalid_jwks: register_int_counter_vec_with_registry!(
                "invalid_jwks",
                "Total number of invalid JWKs",
                &["provider"],
                registry,
            )
            .unwrap(),
            unique_jwks: register_int_counter_vec_with_registry!(
                "unique_jwks",
                "Total number of unique JWKs",
                &["provider"],
                registry,
            )
            .unwrap(),
        }
    }
}

#[derive(Clone)]
pub struct GrpcMetrics {
    inflight_requests: IntGaugeVec,
    num_requests: IntCounterVec,
    request_latency: HistogramVec,
    /// Known gRPC method paths. Paths not in this set are labelled as `"SPAM"`
    /// to prevent unbounded metric cardinality from arbitrary HTTP traffic.
    known_methods: HashSet<&'static str>,
}

impl GrpcMetrics {
    pub fn new(registry: &Registry) -> Self {
        Self {
            inflight_requests: register_int_gauge_vec_with_registry!(
                "authority_grpc_inflight_requests",
                "Total in-flight authority gRPC requests per method",
                &["method"],
                registry,
            )
            .unwrap(),
            num_requests: register_int_counter_vec_with_registry!(
                "authority_grpc_requests",
                "Total authority gRPC requests per method and status code",
                &["method", "status"],
                registry,
            )
            .unwrap(),
            request_latency: register_histogram_vec_with_registry!(
                "authority_grpc_request_latency",
                "Latency of authority gRPC requests per method in seconds",
                &["method"],
                LATENCY_SEC_BUCKETS.to_vec(),
                registry,
            )
            .unwrap(),
            known_methods: VALIDATOR_METHOD_PATHS.iter().copied().collect(),
        }
    }

    /// Returns the path if it is a known gRPC method, or `"SPAM"` otherwise.
    fn sanitize_path<'a>(&self, path: &'a str) -> &'a str {
        if self.known_methods.contains(path) {
            path
        } else {
            SPAM_LABEL
        }
    }
}

impl MetricsCallbackProvider for GrpcMetrics {
    fn on_request(&self, _path: String) {}

    fn on_response(&self, path: String, latency: Duration, _status: u16, grpc_status_code: Code) {
        let method = self.sanitize_path(&path);
        self.num_requests
            .with_label_values(&[method, grpc_code_to_str(grpc_status_code)])
            .inc();
        self.request_latency
            .with_label_values(&[method])
            .observe(latency.as_secs_f64());
    }

    fn on_start(&self, path: &str) {
        let method = self.sanitize_path(path);
        self.inflight_requests.with_label_values(&[method]).inc();
    }

    fn on_drop(&self, path: &str) {
        let method = self.sanitize_path(path);
        self.inflight_requests.with_label_values(&[method]).dec();
    }
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    use iota_metrics::start_prometheus_server;
    use prometheus::{IntCounter, Registry};

    #[tokio::test]
    pub async fn test_metrics_endpoint_with_multiple_registries_add_remove() {
        let port: u16 = 8081;
        let socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), port);

        let registry_service = start_prometheus_server(socket);

        tokio::task::yield_now().await;

        // now add a few registries to the service along side with metrics
        let registry_1 = Registry::new_custom(Some("consensus".to_string()), None).unwrap();
        let counter_1 = IntCounter::new("counter_1", "a sample counter 1").unwrap();
        registry_1.register(Box::new(counter_1)).unwrap();

        let registry_2 = Registry::new_custom(Some("iota".to_string()), None).unwrap();
        let counter_2 = IntCounter::new("counter_2", "a sample counter 2").unwrap();
        registry_2.register(Box::new(counter_2.clone())).unwrap();

        let registry_1_id = registry_service.add(registry_1);
        let _registry_2_id = registry_service.add(registry_2);

        // request the endpoint
        let result = get_metrics(port).await;

        assert!(result.contains(
            "# HELP iota_counter_2 a sample counter 2
# TYPE iota_counter_2 counter
iota_counter_2 0"
        ));

        assert!(result.contains(
            "# HELP consensus_counter_1 a sample counter 1
# TYPE consensus_counter_1 counter
consensus_counter_1 0"
        ));

        // Now remove registry 1
        assert!(registry_service.remove(registry_1_id));

        // AND increase metric 2
        counter_2.inc();

        // Now pull again metrics
        // request the endpoint
        let result = get_metrics(port).await;

        // Registry 1 metrics should not be present anymore
        assert!(!result.contains(
            "# HELP consensus_counter_1 a sample counter 1
# TYPE consensus_counter_1 counter
consensus_counter_1 0"
        ));

        // Registry 2 metric should have increased by 1
        assert!(result.contains(
            "# HELP iota_counter_2 a sample counter 2
# TYPE iota_counter_2 counter
iota_counter_2 1"
        ));
    }

    async fn get_metrics(port: u16) -> String {
        let client = reqwest::Client::new();
        let response = client
            .get(format!("http://127.0.0.1:{port}/metrics"))
            .send()
            .await
            .unwrap();
        response.text().await.unwrap()
    }
}
