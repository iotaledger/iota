// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_metrics::SUBSECOND_LATENCY_SEC_BUCKETS;
use prometheus::{
    GaugeVec, HistogramVec, IntCounterVec, Registry, register_gauge_vec_with_registry,
    register_histogram_vec_with_registry,
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

    /// Current performance score per validator. It is based
    /// on the average latency, risk, staleness over operation types.
    pub(super) performance: GaugeVec,

    /// Current exploration score per validator. It is based
    /// on the effective sample size.
    pub(super) exploration: GaugeVec,
}

impl ValidatorClientMetrics {
    pub fn new(registry: &Registry) -> Self {
        Self {
            observed_latency: register_histogram_vec_with_registry!(
                "validator_client_observed_latency",
                "Client-observed latency of operations per validator",
                &["validator", "operation_type"],
                SUBSECOND_LATENCY_SEC_BUCKETS.to_vec(),
                registry,
            )
            .unwrap(),

            operation_success: register_int_counter_vec_with_registry!(
                "validator_client_operation_success_total",
                "Total successful operations observed by client per validator",
                &["validator", "operation_type"],
                registry,
            )
            .unwrap(),

            operation_failure: register_int_counter_vec_with_registry!(
                "validator_client_operation_failure_total",
                "Total failed operations observed by client per validator",
                &["validator", "operation_type"],
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

            exploration: register_gauge_vec_with_registry!(
                "validator_client_observed_exploration",
                "Current client-observed exploration per validator.",
                &["validator"],
                registry,
            )
            .unwrap(),
        }
    }

    pub fn new_for_tests() -> Self {
        let registry = Registry::new();
        Self::new(&registry)
    }

    pub(super) fn record_interaction_result(
        &self,
        feedback: &super::OperationFeedback,
        score: (f64, f64),
    ) {
        let operation_str = feedback.operation.as_str();
        let labels = &[feedback.display_name.as_str(), operation_str];
        match feedback.result {
            Ok(latency) => {
                self.observed_latency
                    .with_label_values(labels)
                    .observe(latency.as_secs_f64());
                self.operation_success.with_label_values(labels).inc();
            }
            Err(()) => {
                self.operation_failure.with_label_values(labels).inc();
            }
        }
        let (performance, exploration) = score;
        tracing::debug!(
            "Validator {}: performance {} exploration {}",
            feedback.display_name,
            performance,
            exploration
        );
        self.performance
            .with_label_values(&[feedback.display_name.as_str()])
            .set(performance);
        self.exploration
            .with_label_values(&[feedback.display_name.as_str()])
            .set(exploration);
    }
}
