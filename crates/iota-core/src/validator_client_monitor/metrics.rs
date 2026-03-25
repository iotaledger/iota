// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_metrics::{COUNT_BUCKETS, SUBSECOND_LATENCY_SEC_BUCKETS};
use prometheus::{
    GaugeVec, Histogram, HistogramVec, IntCounterVec, Registry, register_gauge_vec_with_registry,
    register_histogram_vec_with_registry, register_histogram_with_registry,
    register_int_counter_vec_with_registry,
};

#[derive(Clone)]
pub struct ValidatorClientMetrics {
    /// Latency of operations per validator
    pub(super) observed_latency: HistogramVec,

    /// Success count per validator and operation type
    pub(super) operation_success: IntCounterVec,

    /// Failure count per validator and operation type
    pub(super) operation_failure: IntCounterVec,

    /// Current performance per validator. The performance is the average
    /// latency of the validator weighted by the reliability of the
    /// validator.
    pub(super) performance: GaugeVec,

    /// Number of low latency validators that got shuffled.
    pub(super) shuffled_validators: Histogram,
}

impl ValidatorClientMetrics {
    pub fn new(registry: &Registry) -> Self {
        Self {
            observed_latency: register_histogram_vec_with_registry!(
                "validator_client_observed_latency",
                "Client-observed latency of operations per validator",
                &["validator", "operation_type", "ping"],
                SUBSECOND_LATENCY_SEC_BUCKETS.to_vec(),
                registry,
            )
            .unwrap(),

            operation_success: register_int_counter_vec_with_registry!(
                "validator_client_operation_success_total",
                "Total successful operations observed by client per validator",
                &["validator", "operation_type", "ping"],
                registry,
            )
            .unwrap(),

            operation_failure: register_int_counter_vec_with_registry!(
                "validator_client_operation_failure_total",
                "Total failed operations observed by client per validator",
                &["validator", "operation_type", "ping"],
                registry,
            )
            .unwrap(),

            performance: register_gauge_vec_with_registry!(
                "validator_client_observed_performance",
                "Current client-observed performance per validator.",
                &["validator"],
                registry,
            )
            .unwrap(),

            shuffled_validators: register_histogram_with_registry!(
                "validator_client_shuffled_validators",
                "Number of low latency validators that got shuffled",
                COUNT_BUCKETS.to_vec(),
                registry,
            )
            .unwrap(),
        }
    }

    pub fn new_for_tests() -> Self {
        let registry = Registry::new();
        Self::new(&registry)
    }

    pub(super) fn record_interaction_result(&self, feedback: &super::OperationFeedback) {
        let operation_str = feedback.operation.as_str();
        let ping_label = feedback.ping.to_string();
        let labels = &[feedback.display_name.as_str(), operation_str, ping_label.as_str()];
        match feedback.result {
            Ok(latency) => {
                self.observed_latency
                    .with_label_values(labels)
                    .observe(latency.as_secs_f64());
                self.operation_success
                    .with_label_values(labels)
                    .inc();
            }
            Err(()) => {
                self.operation_failure
                    .with_label_values(labels)
                    .inc();
            }
        }
    }
}
