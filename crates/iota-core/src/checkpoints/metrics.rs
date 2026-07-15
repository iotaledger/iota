// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use prometheus_filtered::{
    Histogram, IntCounter, IntCounterVec, IntGauge, IntGaugeVec, MetricLevel, Registry,
    register_histogram_with_registry, register_int_counter_vec_with_registry,
    register_int_counter_with_registry, register_int_gauge_vec_with_registry,
    register_int_gauge_with_registry,
};

pub struct CheckpointMetrics {
    pub last_certified_checkpoint: IntGauge,
    pub last_constructed_checkpoint: IntGauge,
    pub checkpoint_errors: IntCounter,
    pub transactions_included_in_checkpoint: IntCounter,
    pub checkpoint_roots_count: IntCounter,
    pub checkpoint_participation: IntCounterVec,
    pub last_received_checkpoint_signatures: IntGaugeVec,
    pub last_sent_checkpoint_signature: IntGauge,
    pub last_skipped_checkpoint_signature_submission: IntGauge,
    pub last_ignored_checkpoint_signature_received: IntGauge,
    pub highest_accumulated_epoch: IntGauge,
    pub checkpoint_creation_latency: Histogram,
    pub commits_per_checkpoint: Histogram,
    pub remote_checkpoint_forks: IntCounter,
    pub split_brain_checkpoint_forks: IntCounter,
    pub last_created_checkpoint_age: Histogram,
    pub last_certified_checkpoint_age: Histogram,
}

impl CheckpointMetrics {
    pub fn new(registry: &Registry) -> Arc<Self> {
        let this = Self {
            last_certified_checkpoint: register_int_gauge_with_registry!(
                "last_certified_checkpoint",
                "Last certified checkpoint",
                registry;
                MetricLevel::Info,
            )
            .unwrap(),
            last_constructed_checkpoint: register_int_gauge_with_registry!(
                "last_constructed_checkpoint",
                "Last constructed checkpoint",
                registry;
                MetricLevel::Info,
            )
            .unwrap(),
            last_created_checkpoint_age: register_histogram_with_registry!(
                "last_created_checkpoint_age",
                "Age of the last created checkpoint",
                iota_metrics::LATENCY_SEC_BUCKETS.to_vec(),
                registry
            ).unwrap(),
            last_certified_checkpoint_age: register_histogram_with_registry!(
                "last_certified_checkpoint_age",
                "Age of the last certified checkpoint",
                iota_metrics::LATENCY_SEC_BUCKETS.to_vec(),
                registry;
                MetricLevel::Info,
            ).unwrap(),
            checkpoint_errors: register_int_counter_with_registry!(
                "checkpoint_errors",
                "Checkpoints errors count",
                registry;
                MetricLevel::Info,
            )
            .unwrap(),
            transactions_included_in_checkpoint: register_int_counter_with_registry!(
                "transactions_included_in_checkpoint",
                "Transactions included in a checkpoint",
                registry;
                MetricLevel::Info,
            )
            .unwrap(),
            checkpoint_roots_count: register_int_counter_with_registry!(
                "checkpoint_roots_count",
                "Number of checkpoint roots received from consensus",
                registry
            )
            .unwrap(),
            checkpoint_participation: register_int_counter_vec_with_registry!(
                "checkpoint_participation",
                "Participation in checkpoint certification by validator",
                &["signer"],
                registry
            )
            .unwrap(),
            last_received_checkpoint_signatures: register_int_gauge_vec_with_registry!(
                "last_received_checkpoint_signatures",
                "Last received checkpoint signatures by validator",
                &["signer"],
                registry
            )
            .unwrap(),
            last_sent_checkpoint_signature: register_int_gauge_with_registry!(
                "last_sent_checkpoint_signature",
                "Last checkpoint signature sent by myself",
                registry
            )
            .unwrap(),
            last_skipped_checkpoint_signature_submission: register_int_gauge_with_registry!(
                "last_skipped_checkpoint_signature_submission",
                "Last checkpoint signature that this validator skipped submitting because it was already certified.",
                registry
            )
            .unwrap(),
            last_ignored_checkpoint_signature_received: register_int_gauge_with_registry!(
                "last_ignored_checkpoint_signature_received",
                "Last received checkpoint signature that this validator ignored because it was already certified.",
                registry
            )
            .unwrap(),
            highest_accumulated_epoch: register_int_gauge_with_registry!(
                "highest_accumulated_epoch",
                "Highest accumulated epoch",
                registry
            )
            .unwrap(),
            checkpoint_creation_latency: register_histogram_with_registry!(
                "checkpoint_creation_latency",
                "Latency from consensus commit timestamp to local checkpoint creation in milliseconds",
                iota_metrics::LATENCY_SEC_BUCKETS.to_vec(),
                registry;
                MetricLevel::Info,
            ).unwrap(),
            commits_per_checkpoint: register_histogram_with_registry!(
                "commits_per_checkpoint",
                "Number of consensus commits coalesced into a single checkpoint",
                vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0],
                registry,
            ).unwrap(),
            remote_checkpoint_forks: register_int_counter_with_registry!(
                "remote_checkpoint_forks",
                "Number of remote checkpoints that forked from local checkpoints",
                registry;
                MetricLevel::Info,
            )
            .unwrap(),
            split_brain_checkpoint_forks: register_int_counter_with_registry!(
                "split_brain_checkpoint_forks",
                "Number of checkpoints that have resulted in a split brain",
                registry;
                MetricLevel::Info,
            )
            .unwrap(),
        };
        Arc::new(this)
    }

    pub fn new_for_tests() -> Arc<Self> {
        Self::new(&Registry::new())
    }
}
